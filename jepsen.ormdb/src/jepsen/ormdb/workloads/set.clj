(ns jepsen.ormdb.workloads.set
  "Append-only set tests for ormdb.
   Tests that all added elements eventually appear in final reads."
  (:require [clojure.tools.logging :refer [info warn error]]
            [jepsen.client :as client]
            [jepsen.checker :as checker]
            [jepsen.checker.timeline :as timeline]
            [jepsen.generator :as gen]
            [jepsen.ormdb.client :as ormdb]))

;; Set checker - verifies all acknowledged adds are visible

(defn set-checker
  "A checker that verifies:
   1. All acknowledged adds appear in the final read
   2. No unexpected elements appear"
  []
  (reify checker/Checker
    (check [this test history opts]
      (let [;; All acknowledged adds
            adds (->> history
                      (filter #(and (= :add (:f %))
                                    (= :ok (:type %))))
                      (map :value)
                      set)
            ;; Final read
            final-read (->> history
                            (filter #(and (= :read (:f %))
                                          (= :ok (:type %))))
                            last
                            :value)
            ;; Check for missing elements (adds that didn't appear)
            missing (clojure.set/difference adds (or final-read #{}))
            ;; Check for unexpected elements
            unexpected (clojure.set/difference (or final-read #{}) adds)]
        {:valid? (and (empty? missing)
                      (empty? unexpected))
         :add-count (count adds)
         :read-count (count final-read)
         :missing missing
         :missing-count (count missing)
         :unexpected unexpected
         :unexpected-count (count unexpected)}))))

;; Client implementation

(defrecord SetClient [node set-name]
  client/Client

  (open! [this test node]
    (assoc this :node node))

  (setup! [this test]
    (info "Setting up set schema on" node)
    (ormdb/setup-test-schema! node))

  (invoke! [this test op]
    (case (:f op)
      :add
      (let [result (ormdb/add-to-set node set-name (:value op))]
        (if (:ok result)
          (assoc op :type :ok)
          (cond
            (= :timeout (:error result))
            (assoc op :type :info :error :timeout)

            (= :connection-refused (:error result))
            (assoc op :type :fail :error :connection-refused)

            :else
            (assoc op :type :fail :error (:error result)))))

      :read
      (let [result (ormdb/read-set node set-name)]
        (if (:ok result)
          (assoc op :type :ok :value (:elements result))
          (assoc op :type :fail :error (:error result))))))

  (teardown! [this test])

  (close! [this test]))

(defn set-client
  "Creates a new set client."
  [set-name]
  (->SetClient nil set-name))

;; Generator functions

(defn add-op
  "Generates an add operation."
  [counter]
  (fn [_ _]
    {:type :invoke :f :add :value (swap! counter inc)}))

(defn read-op
  "Generates a read operation."
  [_ _]
  {:type :invoke :f :read :value nil})

;; Workload definition

(defn workload
  "Append-only set workload.

   Options:
   - :set-name   - Name of the set (default: 'test-set')
   - :rate       - Operations per second (default: 10)
   - :time-limit - Test duration in seconds (default: 60)"
  [opts]
  (let [set-name (:set-name opts "test-set")
        counter (atom 0)]
    {:client (set-client set-name)

     :generator (->> (add-op counter)
                     gen/repeat
                     (gen/stagger (/ (:rate opts 10)))
                     (gen/time-limit (:time-limit opts 60)))

     :final-generator (gen/once {:type :invoke :f :read :value nil})

     :checker (checker/compose
                {:set (set-checker)
                 :timeline (timeline/html)})}))

;; Variant: Set with concurrent reads

(defn concurrent-set-workload
  "Set workload with concurrent reads during adds.

   Options:
   - :set-name      - Name of the set (default: 'test-set')
   - :rate          - Operations per second (default: 10)
   - :time-limit    - Test duration in seconds (default: 60)
   - :read-fraction - Fraction of operations that are reads (default: 0.1)"
  [opts]
  (let [set-name (:set-name opts "test-set")
        counter (atom 0)
        read-fraction (:read-fraction opts 0.1)]
    {:client (set-client set-name)

     :generator (->> (gen/mix [(gen/repeat (add-op counter))
                               (gen/repeat read-op)])
                     (gen/stagger (/ (:rate opts 10)))
                     (gen/time-limit (:time-limit opts 60)))

     :final-generator (gen/once {:type :invoke :f :read :value nil})

     :checker (checker/compose
                {:set (set-checker)
                 :timeline (timeline/html)})}))
