(ns jepsen.ormdb.nemesis
  "Nemesis implementations for fault injection in ormdb tests.
   Includes process killer, network partitions, and clock skew."
  (:require [clojure.tools.logging :refer [info warn error]]
            [jepsen.nemesis :as nemesis]
            [jepsen.nemesis.combined :as nc]
            [jepsen.control :as c]
            [jepsen.control.util :as cu]
            [jepsen.generator :as gen]
            [jepsen.ormdb.db :as db]))

;; Process killer nemesis

(defn process-killer
  "A nemesis that kills and restarts ormdb-server processes.

   Supports operations:
   - :kill - Kill ormdb-server with SIGKILL
   - :start - Restart ormdb-server"
  []
  (reify nemesis/Nemesis
    (setup! [this test] this)

    (invoke! [this test op]
      (case (:f op)
        :kill
        (let [nodes (or (:value op) (take 1 (shuffle (:nodes test))))]
          (doseq [node nodes]
            (c/on node
              (info "Killing ormdb-server on" node)
              (db/kill-server! test node)))
          (assoc op :type :info :value nodes))

        :start
        (let [nodes (or (:value op) (:nodes test))]
          (doseq [node nodes]
            (c/on node
              (info "Starting ormdb-server on" node)
              (db/start-ormdb! test node)))
          (assoc op :type :info :value nodes))

        ;; Default: pass through
        (assoc op :type :info)))

    (teardown! [this test])))

;; Process pause nemesis (SIGSTOP/SIGCONT)

(defn process-pause
  "A nemesis that pauses and resumes ormdb-server processes.

   Supports operations:
   - :pause - Pause ormdb-server with SIGSTOP
   - :resume - Resume ormdb-server with SIGCONT"
  []
  (reify nemesis/Nemesis
    (setup! [this test] this)

    (invoke! [this test op]
      (case (:f op)
        :pause
        (let [nodes (or (:value op) (take 1 (shuffle (:nodes test))))]
          (doseq [node nodes]
            (c/on node
              (info "Pausing ormdb-server on" node)
              (db/pause-server! test node)))
          (assoc op :type :info :value nodes))

        :resume
        (let [nodes (or (:value op) (:nodes test))]
          (doseq [node nodes]
            (c/on node
              (info "Resuming ormdb-server on" node)
              (db/resume-server! test node)))
          (assoc op :type :info :value nodes))

        ;; Default: pass through
        (assoc op :type :info)))

    (teardown! [this test])))

;; Clock skew nemesis

(defn clock-skew
  "A nemesis that adjusts system clocks to create time drift.

   Supports operations:
   - :skew-clock - Adjust clock by a random amount
   - :reset-clock - Reset clock to correct time
   - :strobe-clock - Rapidly fluctuate clock"
  []
  (reify nemesis/Nemesis
    (setup! [this test]
      ;; Disable NTP on all nodes
      (c/on-many (:nodes test)
        (c/su
          (try
            (c/exec :timedatectl :set-ntp :false)
            (catch Exception e
              (warn "Could not disable NTP:" (.getMessage e))))))
      this)

    (invoke! [this test op]
      (case (:f op)
        :skew-clock
        (let [nodes (or (:value op) (take 1 (shuffle (:nodes test))))
              ;; Skew by up to 500ms in either direction
              delta-ms (- (rand-int 1000) 500)]
          (doseq [node nodes]
            (c/on node
              (info "Skewing clock on" node "by" delta-ms "ms")
              (c/su
                (try
                  ;; Use date command to adjust time
                  (if (pos? delta-ms)
                    (c/exec :date :-s (str "+" delta-ms " milliseconds"))
                    (c/exec :date :-s (str delta-ms " milliseconds")))
                  (catch Exception e
                    (warn "Could not skew clock:" (.getMessage e)))))))
          (assoc op :type :info :value {:nodes nodes :delta-ms delta-ms}))

        :reset-clock
        (let [nodes (or (:value op) (:nodes test))]
          (doseq [node nodes]
            (c/on node
              (info "Resetting clock on" node)
              (c/su
                (try
                  ;; Use ntpdate to sync time
                  (c/exec :ntpdate :-b "pool.ntp.org")
                  (catch Exception e
                    (warn "Could not reset clock:" (.getMessage e)))))))
          (assoc op :type :info :value nodes))

        :strobe-clock
        ;; Rapidly fluctuate clock
        (let [nodes (or (:value op) (take 1 (shuffle (:nodes test))))]
          (doseq [node nodes]
            (c/on node
              (info "Strobing clock on" node)
              (c/su
                (try
                  (dotimes [_ 10]
                    (c/exec :date :-s "+100 milliseconds")
                    (Thread/sleep 10)
                    (c/exec :date :-s "-100 milliseconds")
                    (Thread/sleep 10))
                  (catch Exception e
                    (warn "Could not strobe clock:" (.getMessage e)))))))
          (assoc op :type :info :value nodes))

        ;; Default: pass through
        (assoc op :type :info)))

    (teardown! [this test]
      ;; Re-enable NTP on all nodes
      (c/on-many (:nodes test)
        (c/su
          (try
            (c/exec :timedatectl :set-ntp :true)
            (catch Exception e
              (warn "Could not re-enable NTP:" (.getMessage e))))
          (try
            (c/exec :ntpdate :-b "pool.ntp.org")
            (catch Exception e
              (warn "Could not sync time:" (.getMessage e)))))))))

;; Network partition nemesis (uses Jepsen's built-in)

(defn partition-halves
  "Partitions the cluster into two halves."
  []
  (nemesis/partition-random-halves))

(defn partition-one
  "Isolates a single random node from the cluster."
  []
  (nemesis/partition-random-node))

(defn partition-majorities-ring
  "Creates overlapping majority partitions (ring topology)."
  []
  (nemesis/partition-majorities-ring))

;; Combined nemesis

(defn combined-nemesis
  "Creates a combined nemesis that can perform multiple fault types.

   Supports operations from all nemeses:
   - :kill, :start - Process killer
   - :pause, :resume - Process pause
   - :partition-start, :partition-stop - Network partitions
   - :skew-clock, :reset-clock - Clock skew"
  []
  (nemesis/compose
    {#{:kill :start}                     (process-killer)
     #{:pause :resume}                   (process-pause)
     #{:start-partition :stop-partition} (partition-halves)
     #{:skew-clock :reset-clock}         (clock-skew)}))

;; Nemesis generators

(defn kill-generator
  "Generates kill/start events with specified interval."
  [interval]
  (gen/stagger interval
    (gen/seq
      (cycle
        [{:type :info :f :kill}
         {:type :info :f :start}]))))

(defn pause-generator
  "Generates pause/resume events with specified interval."
  [interval]
  (gen/stagger interval
    (gen/seq
      (cycle
        [{:type :info :f :pause}
         {:type :info :f :resume}]))))

(defn partition-generator
  "Generates partition start/stop events with specified interval."
  [interval]
  (gen/stagger interval
    (gen/seq
      (cycle
        [{:type :info :f :start-partition}
         {:type :info :f :stop-partition}]))))

(defn clock-skew-generator
  "Generates clock skew events with specified interval."
  [interval]
  (gen/stagger interval
    (gen/seq
      (cycle
        [{:type :info :f :skew-clock}
         {:type :info :f :reset-clock}]))))

(defn combined-generator
  "Generates a mix of all fault types.

   Options:
   - :interval   - Base interval between faults (default: 30s)
   - :time-limit - Total test duration (default: 300s)"
  [opts]
  (let [interval (:interval opts 30)
        time-limit (:time-limit opts 300)]
    (gen/phases
      ;; Warm-up period (no faults)
      (gen/sleep 10)

      ;; Main testing phase with interleaved faults
      (->> (gen/mix
             [(fn [_] {:type :info :f :kill})
              (fn [_] {:type :info :f :start})
              (fn [_] {:type :info :f :pause})
              (fn [_] {:type :info :f :resume})
              (fn [_] {:type :info :f :start-partition})
              (fn [_] {:type :info :f :stop-partition})
              (fn [_] {:type :info :f :skew-clock})
              (fn [_] {:type :info :f :reset-clock})])
           (gen/stagger interval)
           (gen/time-limit time-limit))

      ;; Recovery period
      (gen/log "Recovering from faults")
      (gen/once {:type :info :f :stop-partition})
      (gen/once {:type :info :f :resume})
      (gen/once {:type :info :f :start})
      (gen/once {:type :info :f :reset-clock})
      (gen/sleep 30))))

;; Nemesis packages (nemesis + generator pairs)

(def none
  "No faults - for baseline testing."
  {:nemesis nemesis/noop
   :generator nil
   :final-generator nil})

(defn kill
  "Process kill/restart faults.

   Options:
   - :interval - Time between kill events (default: 30s)"
  [opts]
  {:nemesis (process-killer)
   :generator (kill-generator (:interval opts 30))
   :final-generator (gen/once {:type :info :f :start})})

(defn pause
  "Process pause/resume faults.

   Options:
   - :interval - Time between pause events (default: 30s)"
  [opts]
  {:nemesis (process-pause)
   :generator (pause-generator (:interval opts 30))
   :final-generator (gen/once {:type :info :f :resume})})

(defn partition
  "Network partition faults.

   Options:
   - :interval - Time between partition events (default: 30s)"
  [opts]
  {:nemesis (partition-halves)
   :generator (partition-generator (:interval opts 30))
   :final-generator (gen/once {:type :info :f :stop-partition})})

(defn clock
  "Clock skew faults.

   Options:
   - :interval - Time between clock events (default: 30s)"
  [opts]
  {:nemesis (clock-skew)
   :generator (clock-skew-generator (:interval opts 30))
   :final-generator (gen/once {:type :info :f :reset-clock})})

(defn combined
  "Combined faults (all types).

   Options:
   - :interval   - Base interval between faults (default: 30s)
   - :time-limit - Total test duration (default: 300s)"
  [opts]
  {:nemesis (combined-nemesis)
   :generator (combined-generator opts)
   :final-generator (gen/phases
                      (gen/once {:type :info :f :stop-partition})
                      (gen/once {:type :info :f :resume})
                      (gen/once {:type :info :f :start})
                      (gen/once {:type :info :f :reset-clock}))})

;; Lookup function for nemesis by name

(defn nemesis-package
  "Returns a nemesis package by name.

   Supported names:
   - :none      - No faults
   - :kill      - Process kill/restart
   - :pause     - Process pause/resume
   - :partition - Network partitions
   - :clock     - Clock skew
   - :combined  - All fault types"
  [name opts]
  (case name
    :none none
    :kill (kill opts)
    :pause (pause opts)
    :partition (partition opts)
    :clock (clock opts)
    :combined (combined opts)
    ;; Default to none
    none))
