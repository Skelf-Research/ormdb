(ns jepsen.ormdb.core
  "Main entry point for ormdb Jepsen tests."
  (:gen-class)
  (:require [clojure.tools.logging :refer [info warn error]]
            [clojure.string :as str]
            [jepsen.cli :as cli]
            [jepsen.core :as jepsen]
            [jepsen.generator :as gen]
            [jepsen.os.debian :as debian]
            [jepsen.tests :as tests]
            [jepsen.ormdb.db :as db]
            [jepsen.ormdb.nemesis :as nemesis]
            [jepsen.ormdb.checker :as checker]
            [jepsen.ormdb.workloads.register :as register]
            [jepsen.ormdb.workloads.bank :as bank]
            [jepsen.ormdb.workloads.set :as set-workload]
            [jepsen.ormdb.workloads.list-append :as list-append]))

;; Workload registry

(def workloads
  "Map of workload names to workload constructors."
  {:register         register/workload
   :register-single  register/single-register-workload
   :register-cas     register/cas-register-workload
   :register-multi   register/multi-register-workload
   :bank             bank/workload
   :bank-snapshot    bank/snapshot-workload
   :set              set-workload/workload
   :set-concurrent   set-workload/concurrent-set-workload
   :list-append      list-append/workload
   :list-append-simple list-append/simple-workload})

(def workload-names
  "List of available workload names."
  (keys workloads))

;; Nemesis registry

(def nemeses
  "Map of nemesis names to nemesis package constructors."
  {:none      (constantly nemesis/none)
   :kill      nemesis/kill
   :pause     nemesis/pause
   :partition nemesis/partition
   :clock     nemesis/clock
   :combined  nemesis/combined})

(def nemesis-names
  "List of available nemesis names."
  (keys nemeses))

;; Test construction

(defn ormdb-test
  "Constructs an ormdb Jepsen test.

   Options:
   - :workload          - Workload name (default: :register)
   - :nemesis           - Nemesis name (default: :none)
   - :time-limit        - Test duration in seconds (default: 60)
   - :rate              - Operations per second (default: 10)
   - :concurrency       - Concurrent clients per node (default: 5)
   - :ormdb-server-binary - Path to ormdb-server binary
   - :ormdb-gateway-binary - Path to ormdb-gateway binary
   - :ormdb-release-url - URL to download ormdb release
   - :nemesis-interval  - Interval between nemesis events (default: 30)"
  [opts]
  (let [workload-name (:workload opts :register)
        nemesis-name (:nemesis opts :none)
        workload-fn (get workloads workload-name)
        nemesis-fn (get nemeses nemesis-name)
        _ (when-not workload-fn
            (throw (IllegalArgumentException.
                     (str "Unknown workload: " workload-name
                          ". Available: " (str/join ", " (map name workload-names))))))
        _ (when-not nemesis-fn
            (throw (IllegalArgumentException.
                     (str "Unknown nemesis: " nemesis-name
                          ". Available: " (str/join ", " (map name nemesis-names))))))
        workload (workload-fn opts)
        nemesis-pkg (nemesis-fn opts)
        test-name (str "ormdb-" (name workload-name)
                       (when (not= nemesis-name :none)
                         (str "-" (name nemesis-name))))]
    (merge tests/noop-test
           opts
           {:name test-name
            :os debian/os
            :db (db/db)
            :client (:client workload)
            :nemesis (:nemesis nemesis-pkg)
            :checker (:checker workload)
            :generator
            (gen/phases
              ;; Phase 1: Normal operations
              (->> (:generator workload)
                   (gen/nemesis (:generator nemesis-pkg))
                   (gen/time-limit (:time-limit opts 60)))

              ;; Phase 2: Recovery
              (gen/log "Healing cluster")
              (when-let [final-nem (:final-generator nemesis-pkg)]
                (gen/nemesis final-nem))
              (gen/sleep 10)

              ;; Phase 3: Final reads
              (gen/log "Performing final reads")
              (when-let [final-gen (:final-generator workload)]
                (gen/clients final-gen)))})))

;; CLI options

(def cli-opts
  "Additional CLI options for ormdb tests."
  [[nil "--workload WORKLOAD" "Workload to run"
    :default :register
    :parse-fn keyword
    :validate [#(contains? workloads %) (str "Must be one of: " (str/join ", " (map name workload-names)))]]

   [nil "--nemesis NEMESIS" "Nemesis to use"
    :default :none
    :parse-fn keyword
    :validate [#(contains? nemeses %) (str "Must be one of: " (str/join ", " (map name nemesis-names)))]]

   [nil "--rate RATE" "Operations per second"
    :default 10
    :parse-fn #(Double/parseDouble %)
    :validate [pos? "Must be positive"]]

   [nil "--nemesis-interval SECONDS" "Seconds between nemesis events"
    :default 30
    :parse-fn #(Long/parseLong %)
    :validate [pos? "Must be positive"]]

   [nil "--ormdb-server-binary PATH" "Path to ormdb-server binary"]

   [nil "--ormdb-gateway-binary PATH" "Path to ormdb-gateway binary"]

   [nil "--ormdb-release-url URL" "URL to download ormdb release"]

   ;; Workload-specific options
   [nil "--accounts N" "Number of accounts for bank workload"
    :default 5
    :parse-fn #(Long/parseLong %)
    :validate [pos? "Must be positive"]]

   [nil "--initial-balance N" "Initial balance for bank accounts"
    :default 100
    :parse-fn #(Long/parseLong %)
    :validate [#(>= % 0) "Must be non-negative"]]

   [nil "--key-count N" "Number of keys for multi-key workloads"
    :default 10
    :parse-fn #(Long/parseLong %)
    :validate [pos? "Must be positive"]]

   [nil "--ops-per-key N" "Operations per key for multi-key workloads"
    :default 100
    :parse-fn #(Long/parseLong %)
    :validate [pos? "Must be positive"]]])

;; Main entry point

(defn -main
  "Main entry point for CLI."
  [& args]
  (cli/run!
    (merge
      (cli/single-test-cmd {:test-fn ormdb-test
                            :opt-spec cli-opts})
      (cli/test-all-cmd {:tests-fn (fn [opts]
                                     (for [workload [:register :bank :set :list-append]
                                           nemesis [:none :kill :partition]]
                                       (ormdb-test (assoc opts
                                                          :workload workload
                                                          :nemesis nemesis))))
                         :opt-spec cli-opts})
      (cli/serve-cmd))
    args))

;; REPL helpers

(defn run-test!
  "Runs a test from the REPL with the given options."
  [opts]
  (jepsen/run! (ormdb-test opts)))

(defn quick-test!
  "Runs a quick smoke test with no faults."
  [& [{:keys [workload time-limit nodes]
       :or {workload :register
            time-limit 30
            nodes ["n1" "n2" "n3" "n4" "n5"]}}]]
  (run-test! {:workload workload
              :nemesis :none
              :time-limit time-limit
              :nodes nodes
              :concurrency 5
              :rate 10}))

(defn stress-test!
  "Runs a stress test with combined faults."
  [& [{:keys [workload time-limit nodes]
       :or {workload :register
            time-limit 300
            nodes ["n1" "n2" "n3" "n4" "n5"]}}]]
  (run-test! {:workload workload
              :nemesis :combined
              :time-limit time-limit
              :nodes nodes
              :concurrency 10
              :rate 50}))
