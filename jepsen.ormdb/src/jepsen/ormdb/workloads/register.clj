(ns jepsen.ormdb.workloads.register
  "Single-register linearizability tests for ormdb.
   Tests that individual key operations (read, write, CAS) are linearizable."
  (:require [clojure.tools.logging :refer [info warn error]]
            [jepsen.client :as client]
            [jepsen.checker :as checker]
            [jepsen.checker.timeline :as timeline]
            [jepsen.generator :as gen]
            [jepsen.independent :as independent]
            [knossos.model :as model]
            [jepsen.ormdb.client :as ormdb]))

;; Generator functions for register operations

(defn r
  "Generates a read operation."
  [_ _]
  {:type :invoke :f :read :value nil})

(defn w
  "Generates a write operation with a random value."
  [_ _]
  {:type :invoke :f :write :value (rand-int 1000)})

(defn cas
  "Generates a compare-and-swap operation."
  [_ _]
  {:type :invoke :f :cas :value [(rand-int 1000) (rand-int 1000)]})

;; Client implementation

(defrecord RegisterClient [node]
  client/Client

  (open! [this test node]
    (assoc this :node node))

  (setup! [this test]
    ;; Setup schema on this node
    (info "Setting up register schema on" node)
    (ormdb/setup-test-schema! node))

  (invoke! [this test op]
    (let [[k v] (if (= :read (:f op))
                  [(:value op) nil]
                  (if (= :cas (:f op))
                    [(:value (first (:value op))) (:value op)]
                    [(:value op) (:value op)]))]
      (case (:f op)
        :read
        (let [result (ormdb/read-register node k)]
          (if (:ok result)
            (assoc op :type :ok :value (:value result))
            (assoc op :type :fail :error (:error result))))

        :write
        (let [result (ormdb/write-register node k v)]
          (if (:ok result)
            (assoc op :type :ok)
            (assoc op :type :fail :error (:error result))))

        :cas
        (let [[expected new-val] (:value op)
              result (ormdb/cas-register node k expected new-val)]
          (cond
            (:ok result)
            (assoc op :type :ok)

            (= :mismatch (:error result))
            (assoc op :type :fail :error :mismatch)

            :else
            (assoc op :type :fail :error (:error result)))))))

  (teardown! [this test])

  (close! [this test]))

(defn register-client
  "Creates a new register client."
  []
  (->RegisterClient nil))

;; Independent register client (for multi-key tests)

(defrecord IndependentRegisterClient [node]
  client/Client

  (open! [this test node]
    (assoc this :node node))

  (setup! [this test]
    (info "Setting up register schema on" node)
    (ormdb/setup-test-schema! node))

  (invoke! [this test op]
    (let [[k op'] (independent/tuple op)
          key-str (str "register-" k)]
      (case (:f op')
        :read
        (let [result (ormdb/read-register node key-str)]
          (if (:ok result)
            (assoc op :type :ok :value (independent/tuple k (:value result)))
            (assoc op :type :fail :error (:error result))))

        :write
        (let [result (ormdb/write-register node key-str (:value op'))]
          (if (:ok result)
            (assoc op :type :ok)
            (assoc op :type :fail :error (:error result))))

        :cas
        (let [[expected new-val] (:value op')
              result (ormdb/cas-register node key-str expected new-val)]
          (cond
            (:ok result)
            (assoc op :type :ok)

            (= :mismatch (:error result))
            (assoc op :type :fail :error :mismatch)

            :else
            (assoc op :type :fail :error (:error result)))))))

  (teardown! [this test])

  (close! [this test]))

(defn independent-register-client
  "Creates a new independent register client for multi-key tests."
  []
  (->IndependentRegisterClient nil))

;; Workload definitions

(defn single-register-workload
  "A workload for testing linearizability of a single register.

   Options:
   - :rate       - Operations per second (default: 10)
   - :time-limit - Test duration in seconds (default: 60)"
  [opts]
  {:client (register-client)
   :generator (->> (gen/mix [r w])
                   (gen/stagger (/ (:rate opts 10)))
                   (gen/time-limit (:time-limit opts 60)))
   :final-generator (gen/once {:type :invoke :f :read :value nil})
   :checker (checker/compose
              {:linear (checker/linearizable
                         {:model (model/register)
                          :algorithm :linear})
               :timeline (timeline/html)})})

(defn cas-register-workload
  "A workload for testing linearizability with compare-and-swap operations.

   Options:
   - :rate       - Operations per second (default: 10)
   - :time-limit - Test duration in seconds (default: 60)"
  [opts]
  {:client (register-client)
   :generator (->> (gen/mix [r w cas])
                   (gen/stagger (/ (:rate opts 10)))
                   (gen/time-limit (:time-limit opts 60)))
   :final-generator (gen/once {:type :invoke :f :read :value nil})
   :checker (checker/compose
              {:linear (checker/linearizable
                         {:model (model/cas-register)
                          :algorithm :linear})
               :timeline (timeline/html)})})

(defn multi-register-workload
  "A workload for testing linearizability across multiple independent registers.

   Options:
   - :rate         - Operations per second (default: 10)
   - :time-limit   - Test duration in seconds (default: 60)
   - :key-count    - Number of independent keys (default: 10)
   - :ops-per-key  - Operations per key (default: 100)"
  [opts]
  (let [key-count (:key-count opts 10)
        ops-per-key (:ops-per-key opts 100)]
    {:client (independent-register-client)
     :generator (independent/concurrent-generator
                  key-count
                  (range)
                  (fn [k]
                    (->> (gen/mix [r w cas])
                         (gen/stagger (/ (:rate opts 10)))
                         (gen/limit ops-per-key))))
     :final-generator (gen/once {:type :invoke :f :read :value nil})
     :checker (checker/compose
                {:linear (independent/checker
                           (checker/linearizable
                             {:model (model/cas-register)
                              :algorithm :linear}))
                 :timeline (timeline/html)})}))

;; Default workload

(defn workload
  "Default register workload with CAS support.

   Options:
   - :rate         - Operations per second (default: 10)
   - :time-limit   - Test duration in seconds (default: 60)
   - :key-count    - Number of independent keys (default: 10)
   - :ops-per-key  - Operations per key (default: 100)
   - :multi-key    - Use independent multi-key workload (default: true)"
  [opts]
  (if (:multi-key opts true)
    (multi-register-workload opts)
    (cas-register-workload opts)))
