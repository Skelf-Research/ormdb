(ns jepsen.ormdb.client
  "HTTP client adapter for communicating with ormdb via ormdb-gateway.
   Provides low-level HTTP operations and high-level entity operations."
  (:require [clojure.tools.logging :refer [info warn error debug]]
            [clj-http.client :as http]
            [cheshire.core :as json]
            [slingshot.slingshot :refer [try+ throw+]]))

;; Configuration
(def default-timeout 5000)  ; 5 seconds
(def gateway-port 8080)

(defn gateway-url
  "Returns the base URL for the ormdb-gateway on a node."
  [node]
  (str "http://" node ":" gateway-port))

;; Low-level HTTP operations

(defn http-post
  "Performs an HTTP POST request with JSON body."
  [url body & [{:keys [timeout] :or {timeout default-timeout}}]]
  (try+
    (let [response (http/post url
                              {:body (json/generate-string body)
                               :content-type :json
                               :accept :json
                               :socket-timeout timeout
                               :connection-timeout timeout
                               :throw-exceptions false})]
      (if (<= 200 (:status response) 299)
        {:ok true
         :body (when-let [b (:body response)]
                 (json/parse-string b true))}
        {:ok false
         :status (:status response)
         :error (or (some-> response :body (json/parse-string true) :error)
                    (str "HTTP " (:status response)))}))
    (catch java.net.SocketTimeoutException e
      {:ok false :error :timeout})
    (catch java.net.ConnectException e
      {:ok false :error :connection-refused})
    (catch Exception e
      {:ok false :error (.getMessage e)})))

(defn http-get
  "Performs an HTTP GET request."
  [url & [{:keys [timeout] :or {timeout default-timeout}}]]
  (try+
    (let [response (http/get url
                             {:accept :json
                              :socket-timeout timeout
                              :connection-timeout timeout
                              :throw-exceptions false})]
      (if (<= 200 (:status response) 299)
        {:ok true
         :body (when-let [b (:body response)]
                 (json/parse-string b true))}
        {:ok false
         :status (:status response)
         :error (str "HTTP " (:status response))}))
    (catch java.net.SocketTimeoutException e
      {:ok false :error :timeout})
    (catch java.net.ConnectException e
      {:ok false :error :connection-refused})
    (catch Exception e
      {:ok false :error (.getMessage e)})))

;; Health check

(defn healthy?
  "Checks if the ormdb-gateway is healthy on a node."
  [node]
  (let [result (http-get (str (gateway-url node) "/health")
                         {:timeout 2000})]
    (:ok result)))

;; Entity operations

(defn query
  "Executes a query against ormdb.

   Options:
   - :entity - The entity type to query
   - :filter - Filter expression (map)
   - :fields - Fields to select (vector)
   - :order - Order specification
   - :limit - Maximum number of results
   - :offset - Offset for pagination
   - :include - Relations to include"
  [node {:keys [entity filter fields order limit offset include]
         :or {fields []}}]
  (let [url (str (gateway-url node) "/query")
        body {:root_entity entity
              :filter filter
              :fields fields
              :order order
              :pagination (when (or limit offset)
                            {:limit limit :offset offset})
              :include include}]
    (http-post url body)))

(defn mutate
  "Executes a single mutation against ormdb.

   Type can be: :insert, :update, :upsert, :delete
   Data is a map of field values."
  [node type entity data & [{:keys [id expected-version]}]]
  (let [url (str (gateway-url node) "/mutate")
        body (cond-> {:type (name type)
                      :entity entity
                      :data data}
               id (assoc :id id)
               expected-version (assoc :expected_version expected-version))]
    (http-post url body)))

(defn mutate-batch
  "Executes a batch of mutations atomically.

   Mutations is a vector of mutation maps, each with:
   - :type - :insert, :update, :upsert, :delete
   - :entity - Entity type
   - :data - Field values
   - :id - Entity ID (for update/delete)"
  [node mutations]
  (let [url (str (gateway-url node) "/mutate-batch")
        body {:mutations (mapv (fn [m]
                                 (cond-> {:type (name (:type m))
                                          :entity (:entity m)}
                                   (:data m) (assoc :data (:data m))
                                   (:id m) (assoc :id (:id m))
                                   (:expected-version m) (assoc :expected_version (:expected-version m))))
                               mutations)}]
    (http-post url body)))

;; Convenience operations for test workloads

(defn extract-rows
  "Extracts rows from the first entity block in a query response.
   Response format: {:body {:data {:entities [{:entity \"...\", :rows [...]}]}}}"
  [result]
  (get-in result [:body :data :entities 0 :rows]))

(defn read-register
  "Reads a register value by key."
  [node key]
  (let [result (query node {:entity "Register"
                            :filter {:key key}
                            :fields ["key" "value"]})]
    (if (:ok result)
      (let [rows (extract-rows result)]
        (if (seq rows)
          {:ok true :value (:value (first rows))}
          {:ok true :value nil}))
      result)))

(defn write-register
  "Writes a value to a register by key."
  [node key value]
  (mutate node :upsert "Register" {:key key :value value}))

(defn cas-register
  "Performs compare-and-swap on a register.
   Returns {:ok true} if successful, {:ok false :error :mismatch} if value didn't match."
  [node key expected new-value]
  (let [current (read-register node key)]
    (if (:ok current)
      (if (= (:value current) expected)
        (write-register node key new-value)
        {:ok false :error :mismatch :current (:value current)})
      current)))

(defn read-account
  "Reads an account balance."
  [node account-id]
  (let [result (query node {:entity "Account"
                            :filter {:id account-id}
                            :fields ["id" "balance"]})]
    (if (:ok result)
      (let [rows (extract-rows result)]
        (if (seq rows)
          {:ok true :balance (:balance (first rows))}
          {:ok true :balance nil}))
      result)))

(defn read-all-accounts
  "Reads all account balances."
  [node]
  (let [result (query node {:entity "Account"
                            :fields ["id" "balance"]})]
    (if (:ok result)
      {:ok true
       :balances (into {}
                       (map (fn [e] [(:id e) (:balance e)])
                            (extract-rows result)))}
      result)))

(defn transfer
  "Transfers amount from one account to another atomically."
  [node from-account to-account amount]
  (mutate-batch node
                [{:type :update
                  :entity "Account"
                  :id from-account
                  :data {:balance {:$decr amount}}}
                 {:type :update
                  :entity "Account"
                  :id to-account
                  :data {:balance {:$incr amount}}}]))

(defn add-to-set
  "Adds an element to a set entity."
  [node set-name element]
  (mutate node :insert "SetElement" {:set_name set-name :value element}))

(defn read-set
  "Reads all elements from a set."
  [node set-name]
  (let [result (query node {:entity "SetElement"
                            :filter {:set_name set-name}
                            :fields ["value"]})]
    (if (:ok result)
      {:ok true
       :elements (set (map :value (extract-rows result)))}
      result)))

(defn append-to-list
  "Appends a value to a list."
  [node list-key value]
  (mutate node :upsert "List" {:key list-key :values {:$push value}}))

(defn read-list
  "Reads a list by key."
  [node list-key]
  (let [result (query node {:entity "List"
                            :filter {:key list-key}
                            :fields ["key" "values"]})]
    (if (:ok result)
      (let [rows (extract-rows result)]
        (if (seq rows)
          {:ok true :values (:values (first rows))}
          {:ok true :values []}))
      result)))

(defn increment-counter
  "Increments a counter."
  [node counter-id]
  (mutate node :upsert "Counter" {:id counter-id :value {:$incr 1}}))

(defn read-counter
  "Reads a counter value."
  [node counter-id]
  (let [result (query node {:entity "Counter"
                            :filter {:id counter-id}
                            :fields ["id" "value"]})]
    (if (:ok result)
      (let [rows (extract-rows result)]
        (if (seq rows)
          {:ok true :value (:value (first rows))}
          {:ok true :value 0}))
      result)))

;; Schema operations

(defn get-schema
  "Retrieves the current schema from ormdb."
  [node]
  (http-get (str (gateway-url node) "/schema")))

(defn apply-schema
  "Applies a schema to ormdb."
  [node schema]
  (http-post (str (gateway-url node) "/schema") schema))

;; Test schema for Jepsen workloads

(def test-schema
  "Schema for Jepsen test entities."
  {:entities
   [{:name "Register"
     :identity_field "key"
     :fields [{:name "key" :type "string"}
              {:name "value" :type "int"}]}
    {:name "Account"
     :identity_field "id"
     :fields [{:name "id" :type "int"}
              {:name "balance" :type "int"}]}
    {:name "SetElement"
     :identity_field "id"
     :fields [{:name "id" :type "uuid"}
              {:name "set_name" :type "string"}
              {:name "value" :type "int"}]}
    {:name "List"
     :identity_field "key"
     :fields [{:name "key" :type "int"}
              {:name "values" :type "int[]"}]}
    {:name "Counter"
     :identity_field "id"
     :fields [{:name "id" :type "string"}
              {:name "value" :type "int"}]}]})

(defn setup-test-schema!
  "Sets up the test schema on a node."
  [node]
  (apply-schema node test-schema))
