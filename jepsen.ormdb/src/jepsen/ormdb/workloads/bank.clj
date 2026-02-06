(ns jepsen.ormdb.workloads.bank
  "Bank transfer tests for ormdb serializability.
   Tests that transfers between accounts maintain total balance invariant."
  (:require [clojure.tools.logging :refer [info warn error]]
            [jepsen.client :as client]
            [jepsen.checker :as checker]
            [jepsen.checker.timeline :as timeline]
            [jepsen.generator :as gen]
            [jepsen.ormdb.client :as ormdb]))

;; Bank checker - verifies total balance is conserved

(defn bank-checker
  "A checker that verifies:
   1. All reads see the same total balance
   2. No negative balances
   3. Total balance equals initial total"
  [initial-balance account-count]
  (let [expected-total (* initial-balance account-count)]
    (reify checker/Checker
      (check [this test history opts]
        (let [reads (->> history
                         (filter #(and (= :ok (:type %))
                                       (= :read (:f %))))
                         (map :value))
              totals (map (fn [balances]
                            (reduce + (vals balances)))
                          reads)
              negative-balances (->> reads
                                     (mapcat vals)
                                     (filter neg?))
              wrong-totals (filter #(not= expected-total %) totals)]
          {:valid? (and (empty? negative-balances)
                        (empty? wrong-totals))
           :expected-total expected-total
           :read-count (count reads)
           :wrong-totals (take 10 wrong-totals)
           :negative-balances (take 10 negative-balances)})))))

;; Client implementation

(defrecord BankClient [node accounts initial-balance]
  client/Client

  (open! [this test node]
    (assoc this :node node))

  (setup! [this test]
    (info "Setting up bank accounts on" node)
    ;; Setup schema
    (ormdb/setup-test-schema! node)
    ;; Create initial accounts with balances
    (doseq [acct (range accounts)]
      (let [result (ormdb/mutate node :insert "Account"
                                 {:id acct :balance initial-balance})]
        (when-not (:ok result)
          (warn "Failed to create account" acct ":" (:error result))))))

  (invoke! [this test op]
    (case (:f op)
      :read
      (let [result (ormdb/read-all-accounts node)]
        (if (:ok result)
          (assoc op :type :ok :value (:balances result))
          (assoc op :type :fail :error (:error result))))

      :transfer
      (let [{:keys [from to amount]} (:value op)
            result (ormdb/transfer node from to amount)]
        (if (:ok result)
          (assoc op :type :ok)
          ;; Check for specific errors
          (cond
            (= :timeout (:error result))
            (assoc op :type :info :error :timeout)

            (= :connection-refused (:error result))
            (assoc op :type :fail :error :connection-refused)

            ;; Negative balance constraint violation
            (and (string? (:error result))
                 (re-find #"(?i)constraint|negative|insufficient" (:error result)))
            (assoc op :type :fail :error :negative-balance)

            :else
            (assoc op :type :fail :error (:error result)))))))

  (teardown! [this test])

  (close! [this test]))

(defn bank-client
  "Creates a new bank client."
  [accounts initial-balance]
  (->BankClient nil accounts initial-balance))

;; Generator functions

(defn read-op
  "Generates a read operation."
  [_ _]
  {:type :invoke :f :read :value nil})

(defn transfer-op
  "Generates a transfer operation."
  [accounts max-transfer]
  (fn [_ _]
    (let [from (rand-int accounts)
          to (loop [t (rand-int accounts)]
               (if (= t from)
                 (recur (rand-int accounts))
                 t))
          amount (inc (rand-int max-transfer))]
      {:type :invoke
       :f :transfer
       :value {:from from :to to :amount amount}})))

;; Workload definition

(defn workload
  "Bank transfer workload for testing serializability.

   Options:
   - :accounts        - Number of accounts (default: 5)
   - :initial-balance - Starting balance per account (default: 100)
   - :max-transfer    - Maximum transfer amount (default: 50)
   - :rate            - Operations per second (default: 10)
   - :time-limit      - Test duration in seconds (default: 60)
   - :read-fraction   - Fraction of operations that are reads (default: 0.2)"
  [opts]
  (let [accounts (:accounts opts 5)
        initial-balance (:initial-balance opts 100)
        max-transfer (:max-transfer opts 50)
        read-fraction (:read-fraction opts 0.2)]
    {:client (bank-client accounts initial-balance)

     :generator (->> (gen/mix [(gen/repeat read-op)
                               (gen/repeat (transfer-op accounts max-transfer))])
                     (gen/stagger (/ (:rate opts 10)))
                     (gen/time-limit (:time-limit opts 60)))

     :final-generator (gen/once {:type :invoke :f :read :value nil})

     :checker (checker/compose
                {:bank (bank-checker initial-balance accounts)
                 :timeline (timeline/html)})}))

;; Variant: Bank with reads during transfers (snapshot isolation test)

(defrecord BankSnapshotClient [node accounts initial-balance]
  client/Client

  (open! [this test node]
    (assoc this :node node))

  (setup! [this test]
    (info "Setting up bank accounts on" node)
    (ormdb/setup-test-schema! node)
    (doseq [acct (range accounts)]
      (ormdb/mutate node :insert "Account" {:id acct :balance initial-balance})))

  (invoke! [this test op]
    (case (:f op)
      :read
      (let [result (ormdb/read-all-accounts node)]
        (if (:ok result)
          (assoc op :type :ok :value (:balances result))
          (assoc op :type :fail :error (:error result))))

      :transfer-and-read
      ;; Perform a transfer and read in a transaction-like sequence
      ;; This tests snapshot isolation - reads during transfer should see
      ;; consistent state
      (let [{:keys [from to amount]} (:value op)
            ;; Read before
            before (ormdb/read-all-accounts node)
            ;; Transfer
            transfer-result (ormdb/transfer node from to amount)
            ;; Read after
            after (ormdb/read-all-accounts node)]
        (if (and (:ok before) (:ok transfer-result) (:ok after))
          (assoc op :type :ok
                 :value {:before (:balances before)
                         :after (:balances after)
                         :transfer {:from from :to to :amount amount}})
          (assoc op :type :fail
                 :error (or (:error transfer-result)
                            (:error before)
                            (:error after)))))))

  (teardown! [this test])

  (close! [this test]))

(defn snapshot-checker
  "Checks that reads during transfers see consistent total."
  [initial-balance account-count]
  (let [expected-total (* initial-balance account-count)]
    (reify checker/Checker
      (check [this test history opts]
        (let [transfer-reads (->> history
                                  (filter #(and (= :ok (:type %))
                                                (= :transfer-and-read (:f %))))
                                  (map :value))
              check-totals (fn [{:keys [before after]}]
                             (let [before-total (reduce + (vals before))
                                   after-total (reduce + (vals after))]
                               (and (= expected-total before-total)
                                    (= expected-total after-total))))
              inconsistent (filter (complement check-totals) transfer-reads)]
          {:valid? (empty? inconsistent)
           :expected-total expected-total
           :inconsistent-count (count inconsistent)
           :examples (take 5 inconsistent)})))))

(defn snapshot-workload
  "Workload for testing snapshot isolation during transfers.

   Options:
   - :accounts        - Number of accounts (default: 5)
   - :initial-balance - Starting balance per account (default: 100)
   - :max-transfer    - Maximum transfer amount (default: 50)
   - :rate            - Operations per second (default: 10)
   - :time-limit      - Test duration in seconds (default: 60)"
  [opts]
  (let [accounts (:accounts opts 5)
        initial-balance (:initial-balance opts 100)
        max-transfer (:max-transfer opts 50)]
    {:client (->BankSnapshotClient nil accounts initial-balance)

     :generator (->> (gen/mix
                       [(gen/repeat read-op)
                        (gen/repeat
                          (fn [_ _]
                            (let [from (rand-int accounts)
                                  to (loop [t (rand-int accounts)]
                                       (if (= t from)
                                         (recur (rand-int accounts))
                                         t))
                                  amount (inc (rand-int max-transfer))]
                              {:type :invoke
                               :f :transfer-and-read
                               :value {:from from :to to :amount amount}})))])
                     (gen/stagger (/ (:rate opts 10)))
                     (gen/time-limit (:time-limit opts 60)))

     :final-generator (gen/once {:type :invoke :f :read :value nil})

     :checker (checker/compose
                {:snapshot (snapshot-checker initial-balance accounts)
                 :timeline (timeline/html)})}))
