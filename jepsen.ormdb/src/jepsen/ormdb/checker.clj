(ns jepsen.ormdb.checker
  "Custom checkers for ormdb Jepsen tests."
  (:require [clojure.tools.logging :refer [info warn error]]
            [jepsen.checker :as checker]
            [jepsen.checker.timeline :as timeline]
            [jepsen.checker.perf :as perf]
            [knossos.model :as model]))

;; Composite checker that includes common checks

(defn standard-checker
  "Returns a checker that includes:
   - Performance metrics (latency, throughput)
   - Timeline visualization
   - Clock synchronization check

   Additional checkers should be composed with this."
  []
  (checker/compose
    {:perf (checker/perf)
     :timeline (timeline/html)
     :clock (checker/clock-plot)
     :stats (checker/stats)}))

;; Linearizability checker wrapper

(defn linearizable
  "Returns a linearizability checker for register operations.

   Options:
   - :model     - The knossos model (default: cas-register)
   - :algorithm - The checking algorithm (default: :linear)"
  [& [{:keys [model algorithm] :or {model (model/cas-register)
                                     algorithm :linear}}]]
  (checker/linearizable {:model model
                         :algorithm algorithm}))

;; Comprehensive checker combining multiple checks

(defn comprehensive
  "Returns a comprehensive checker suitable for most tests.

   Combines:
   - Linearizability (optional, if :linearizable? is true)
   - Performance metrics
   - Timeline
   - Clock synchronization
   - Statistics

   Options:
   - :linearizable? - Include linearizability check (default: false)
   - :model         - Model for linearizability (default: cas-register)"
  [& [{:keys [linearizable? model] :or {linearizable? false
                                         model (model/cas-register)}}]]
  (if linearizable?
    (checker/compose
      {:linear (linearizable {:model model})
       :perf (checker/perf)
       :timeline (timeline/html)
       :clock (checker/clock-plot)
       :stats (checker/stats)})
    (standard-checker)))

;; Operation counting checker

(defn operation-counter
  "A checker that counts operations by type and status."
  []
  (reify checker/Checker
    (check [this test history opts]
      (let [by-type-status (group-by (juxt :f :type) history)
            counts (into {}
                         (map (fn [[[f t] ops]]
                                [(str (name f) "-" (name t)) (count ops)])
                              by-type-status))
            total (count history)
            ok-count (count (filter #(= :ok (:type %)) history))
            fail-count (count (filter #(= :fail (:type %)) history))
            info-count (count (filter #(= :info (:type %)) history))]
        {:valid? true
         :total-ops total
         :ok-count ok-count
         :fail-count fail-count
         :info-count info-count
         :by-operation counts}))))

;; Latency checker

(defn latency-checker
  "A checker that analyzes operation latencies."
  []
  (reify checker/Checker
    (check [this test history opts]
      (let [completed (->> history
                           (filter #(#{:ok :fail :info} (:type %)))
                           (filter :time))
            latencies (->> completed
                           (map (fn [op]
                                  (when-let [invoke (some #(when (and (= :invoke (:type %))
                                                                      (= (:process %) (:process op)))
                                                             %)
                                                         (reverse (take-while #(not= op %) history)))]
                                    (- (:time op) (:time invoke)))))
                           (filter some?)
                           (map #(/ % 1e6)))  ; Convert to milliseconds
            sorted-latencies (sort latencies)
            n (count sorted-latencies)]
        (if (pos? n)
          {:valid? true
           :count n
           :min-ms (first sorted-latencies)
           :max-ms (last sorted-latencies)
           :mean-ms (/ (reduce + sorted-latencies) n)
           :p50-ms (nth sorted-latencies (int (* 0.5 n)) 0)
           :p95-ms (nth sorted-latencies (int (* 0.95 n)) 0)
           :p99-ms (nth sorted-latencies (int (* 0.99 n)) 0)}
          {:valid? true
           :count 0})))))

;; Availability checker

(defn availability-checker
  "A checker that measures system availability during the test."
  []
  (reify checker/Checker
    (check [this test history opts]
      (let [invokes (filter #(= :invoke (:type %)) history)
            completions (filter #(#{:ok :fail :info} (:type %)) history)
            successful (filter #(= :ok (:type %)) completions)
            failed (filter #(= :fail (:type %)) completions)
            indeterminate (filter #(= :info (:type %)) completions)
            total (count invokes)
            success-rate (if (pos? total)
                           (double (/ (count successful) total))
                           1.0)
            fail-rate (if (pos? total)
                        (double (/ (count failed) total))
                        0.0)]
        {:valid? (>= success-rate 0.5)  ; Consider valid if >50% success
         :total-invokes total
         :successful (count successful)
         :failed (count failed)
         :indeterminate (count indeterminate)
         :success-rate success-rate
         :fail-rate fail-rate}))))

;; Durability checker (for checking that acknowledged writes persist)

(defn durability-checker
  "A checker that verifies acknowledged writes appear in final reads.

   Requires the workload to provide:
   - :final-read? metadata on read operations
   - :write-key and :write-value on write operations"
  []
  (reify checker/Checker
    (check [this test history opts]
      (let [;; All acknowledged writes
            writes (->> history
                        (filter #(and (= :ok (:type %))
                                      (#{:write :insert :append :add} (:f %))))
                        (map (fn [op]
                               {:key (or (:write-key op) (:key (:value op)))
                                :value (or (:write-value op) (:value op))})))
            ;; Final reads (operations with :final-read? true)
            final-reads (->> history
                             (filter #(and (= :ok (:type %))
                                           (= :read (:f %))
                                           (:final-read? %))))]
        (if (seq final-reads)
          (let [final-values (into {} (map (fn [op]
                                             [(:key (:value op)) (:value (:value op))])
                                           final-reads))
                missing (filter (fn [{:keys [key value]}]
                                  (not= value (get final-values key)))
                                writes)]
            {:valid? (empty? missing)
             :write-count (count writes)
             :missing-count (count missing)
             :missing (take 10 missing)})
          {:valid? :unknown
           :note "No final reads found"})))))

;; Combining checkers for different workloads

(defn register-checker
  "Checker for register workloads."
  [opts]
  (checker/compose
    {:linear (linearizable opts)
     :perf (checker/perf)
     :timeline (timeline/html)
     :stats (checker/stats)
     :ops (operation-counter)
     :latency (latency-checker)
     :availability (availability-checker)}))

(defn bank-checker
  "Checker for bank workloads."
  [bank-checker-impl]
  (checker/compose
    {:bank bank-checker-impl
     :perf (checker/perf)
     :timeline (timeline/html)
     :stats (checker/stats)
     :ops (operation-counter)
     :availability (availability-checker)}))

(defn set-checker
  "Checker for set workloads."
  [set-checker-impl]
  (checker/compose
    {:set set-checker-impl
     :perf (checker/perf)
     :timeline (timeline/html)
     :stats (checker/stats)
     :ops (operation-counter)
     :availability (availability-checker)}))

(defn list-append-checker
  "Checker for list-append workloads."
  [elle-checker]
  (checker/compose
    {:elle elle-checker
     :perf (checker/perf)
     :timeline (timeline/html)
     :stats (checker/stats)
     :ops (operation-counter)
     :availability (availability-checker)}))
