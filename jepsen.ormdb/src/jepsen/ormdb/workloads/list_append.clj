(ns jepsen.ormdb.workloads.list-append
  "List-append transactional tests for ormdb using Elle.
   Tests strict serializability through list append operations."
  (:require [clojure.tools.logging :refer [info warn error]]
            [jepsen.client :as client]
            [jepsen.checker :as checker]
            [jepsen.checker.timeline :as timeline]
            [jepsen.generator :as gen]
            [jepsen.tests.cycle :as cycle]
            [jepsen.tests.cycle.append :as append]
            [jepsen.ormdb.client :as ormdb]))

;; Client implementation for list-append workload

(defrecord ListAppendClient [node]
  client/Client

  (open! [this test node]
    (assoc this :node node))

  (setup! [this test]
    (info "Setting up list schema on" node)
    (ormdb/setup-test-schema! node))

  (invoke! [this test op]
    ;; op has :value which is a transaction - a vector of [f k v] micro-ops
    ;; where f is :r (read) or :append
    (let [txn (:value op)]
      (try
        (let [results
              (mapv (fn [[f k v]]
                      (case f
                        :r
                        (let [result (ormdb/read-list node k)]
                          (if (:ok result)
                            [:r k (:values result)]
                            (throw (ex-info "Read failed" {:key k :error (:error result)}))))

                        :append
                        (let [result (ormdb/append-to-list node k v)]
                          (if (:ok result)
                            [:append k v]
                            (throw (ex-info "Append failed" {:key k :value v :error (:error result)}))))))
                    txn)]
          (assoc op :type :ok :value results))
        (catch clojure.lang.ExceptionInfo e
          (let [data (ex-data e)]
            (cond
              (= :timeout (:error data))
              (assoc op :type :info :error :timeout)

              (= :connection-refused (:error data))
              (assoc op :type :fail :error :connection-refused)

              :else
              (assoc op :type :fail :error (:error data)))))
        (catch Exception e
          (assoc op :type :fail :error (.getMessage e))))))

  (teardown! [this test])

  (close! [this test]))

(defn list-append-client
  "Creates a new list-append client."
  []
  (->ListAppendClient nil))

;; Workload definition using Elle's append generator and checker

(defn workload
  "List-append workload for testing strict serializability using Elle.

   This workload generates transactions that read and append to lists,
   then uses Elle to verify that the observed history is consistent
   with strict serializability.

   Options:
   - :key-count          - Number of distinct keys (default: 5)
   - :min-txn-length     - Minimum operations per transaction (default: 1)
   - :max-txn-length     - Maximum operations per transaction (default: 4)
   - :max-writes-per-key - Maximum appends per key (default: 32)
   - :rate               - Operations per second (default: 10)
   - :time-limit         - Test duration in seconds (default: 60)
   - :consistency-models - Models to check (default: [:strict-serializable])"
  [opts]
  (let [key-count (:key-count opts 5)
        min-txn-length (:min-txn-length opts 1)
        max-txn-length (:max-txn-length opts 4)
        max-writes-per-key (:max-writes-per-key opts 32)
        consistency-models (:consistency-models opts [:strict-serializable])]
    {:client (list-append-client)

     :generator (->> (append/gen {:key-count key-count
                                  :min-txn-length min-txn-length
                                  :max-txn-length max-txn-length
                                  :max-writes-per-key max-writes-per-key})
                     (gen/stagger (/ (:rate opts 10)))
                     (gen/time-limit (:time-limit opts 60)))

     :final-generator nil  ;; Elle uses the full history

     :checker (checker/compose
                {:elle (append/checker {:consistency-models consistency-models})
                 :timeline (timeline/html)})}))

;; Variant workloads for different consistency models

(defn serializable-workload
  "List-append workload checking for serializability (not strict)."
  [opts]
  (workload (assoc opts :consistency-models [:serializable])))

(defn snapshot-isolation-workload
  "List-append workload checking for snapshot isolation."
  [opts]
  (workload (assoc opts :consistency-models [:snapshot-isolation])))

(defn read-committed-workload
  "List-append workload checking for read committed."
  [opts]
  (workload (assoc opts :consistency-models [:read-committed])))

;; Simple list-append without Elle (for debugging)

(defn simple-list-checker
  "A simple checker that verifies list contents are valid appends."
  []
  (reify checker/Checker
    (check [this test history opts]
      (let [;; Group operations by key
            ops-by-key (->> history
                            (filter #(= :ok (:type %)))
                            (mapcat (fn [op]
                                      (map (fn [[f k v]] {:op-type f :key k :value v})
                                           (:value op))))
                            (group-by :key))
            ;; For each key, verify that reads see prefixes of appends
            check-key (fn [k ops]
                        (let [appends (->> ops
                                           (filter #(= :append (:op-type %)))
                                           (map :value))
                              reads (->> ops
                                         (filter #(= :r (:op-type %)))
                                         (map :value))]
                          ;; Each read should be a prefix of the append sequence
                          (every? (fn [read-val]
                                    (or (nil? read-val)
                                        (empty? read-val)
                                        (= read-val (take (count read-val) appends))))
                                  reads)))
            results (map (fn [[k ops]] [k (check-key k ops)]) ops-by-key)
            invalid-keys (filter (fn [[k valid?]] (not valid?)) results)]
        {:valid? (empty? invalid-keys)
         :key-count (count ops-by-key)
         :invalid-keys (map first invalid-keys)}))))

(defn simple-workload
  "Simpler list-append workload without Elle for basic testing.

   Options:
   - :key-count  - Number of distinct keys (default: 5)
   - :rate       - Operations per second (default: 10)
   - :time-limit - Test duration in seconds (default: 60)"
  [opts]
  (let [key-count (:key-count opts 5)
        counter (atom 0)]
    {:client (list-append-client)

     :generator (->> (fn [_ _]
                       (let [k (rand-int key-count)
                             f (if (< (rand) 0.3) :r :append)]
                         {:type :invoke
                          :f :txn
                          :value (if (= f :r)
                                   [[:r k nil]]
                                   [[:append k (swap! counter inc)]])}))
                     gen/repeat
                     (gen/stagger (/ (:rate opts 10)))
                     (gen/time-limit (:time-limit opts 60)))

     :final-generator (gen/once {:type :invoke
                                 :f :txn
                                 :value (vec (for [k (range key-count)]
                                               [:r k nil]))})

     :checker (checker/compose
                {:list (simple-list-checker)
                 :timeline (timeline/html)})}))
