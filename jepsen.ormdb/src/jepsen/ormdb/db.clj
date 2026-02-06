(ns jepsen.ormdb.db
  "Database lifecycle management for ORMDB Jepsen tests.
   Handles installation, configuration, startup, and teardown of ormdb nodes."
  (:require [clojure.tools.logging :refer [info warn error]]
            [clojure.string :as str]
            [cheshire.core :as json]
            [jepsen.control :as c]
            [jepsen.control.util :as cu]
            [jepsen.db :as db]
            [jepsen.os.debian :as debian]))

;; Directory constants
(def ormdb-dir "/opt/ormdb")
(def data-dir "/var/lib/ormdb")
(def log-dir "/var/log/ormdb")
(def config-file "/etc/ormdb/config.json")

(def server-bin (str ormdb-dir "/ormdb-server"))
(def gateway-bin (str ormdb-dir "/ormdb-gateway"))

(def server-pidfile "/var/run/ormdb-server.pid")
(def gateway-pidfile "/var/run/ormdb-gateway.pid")

(def server-logfile (str log-dir "/ormdb-server.log"))
(def gateway-logfile (str log-dir "/ormdb-gateway.log"))

;; Ports
(def server-port 9000)
(def gateway-port 8080)
(def raft-base-port 9100)

(defn node-id
  "Returns the numeric ID for a node based on its position in the cluster."
  [test node]
  (.indexOf (:nodes test) node))

(defn raft-port
  "Returns the Raft port for a given node."
  [test node]
  (+ raft-base-port (node-id test node)))

(defn raft-config
  "Generates Raft cluster configuration for a node."
  [test node]
  (let [nodes (:nodes test)
        nid (node-id test node)]
    {:node_id nid
     :listen_addr (str "0.0.0.0:" server-port)
     :raft_addr (str "0.0.0.0:" (raft-port test node))
     :advertise_addr (str node ":" server-port)
     :raft_advertise_addr (str node ":" (raft-port test node))
     :data_dir data-dir
     :peers (vec (for [n nodes
                       :when (not= n node)]
                   {:id (node-id test n)
                    :addr (str n ":" (raft-port test n))}))
     :election_timeout_ms 150
     :heartbeat_interval_ms 50
     :snapshot_threshold 1000
     :max_payload_size 1048576}))

(defn install-ormdb!
  "Installs ormdb binaries on a node."
  [test node]
  (info node "Installing ormdb")
  (c/su
    ;; Create directories
    (c/exec :mkdir :-p ormdb-dir)
    (c/exec :mkdir :-p data-dir)
    (c/exec :mkdir :-p log-dir)
    (c/exec :mkdir :-p "/etc/ormdb")

    ;; Upload binaries from test config
    (when-let [server-binary (:ormdb-server-binary test)]
      (c/upload server-binary server-bin)
      (c/exec :chmod "+x" server-bin))

    (when-let [gateway-binary (:ormdb-gateway-binary test)]
      (c/upload gateway-binary gateway-bin)
      (c/exec :chmod "+x" gateway-bin))

    ;; If no binaries provided, try to download from release URL
    (when (and (nil? (:ormdb-server-binary test))
               (:ormdb-release-url test))
      (let [url (:ormdb-release-url test)]
        (c/exec :curl :-L :-o "/tmp/ormdb.tar.gz" url)
        (c/exec :tar :-xzf "/tmp/ormdb.tar.gz" :-C ormdb-dir)
        (c/exec :chmod "+x" server-bin)
        (c/exec :chmod "+x" gateway-bin)))))

(defn write-config!
  "Writes the ormdb configuration file for a node."
  [test node]
  (info node "Writing ormdb config")
  (c/su
    (let [config (raft-config test node)
          config-json (json/generate-string config {:pretty true})]
      (c/exec :echo config-json :> config-file))))

(defn cluster-members-str
  "Generates cluster members string for initialization.
   Format: id:addr,id:addr,..."
  [test]
  (str/join ","
            (for [n (:nodes test)]
              (str (node-id test n) ":" n ":" (raft-port test n)))))

(defn first-node?
  "Returns true if this is the first node in the cluster."
  [test node]
  (= node (first (:nodes test))))

(defn start-server!
  "Starts the ormdb-server process on a node."
  [test node]
  (info node "Starting ormdb-server")
  (let [nid (node-id test node)
        raft-addr (str "0.0.0.0:" (raft-port test node))
        raft-advertise (str node ":" (raft-port test node))
        is-bootstrap (first-node? test node)
        base-args [:--data-path data-dir
                   :--tcp (str "tcp://0.0.0.0:" server-port)
                   :--raft-node-id nid
                   :--raft-listen raft-addr
                   :--raft-advertise raft-advertise]
        ;; First node initializes the cluster
        init-args (if is-bootstrap
                    [:--cluster-init
                     :--cluster-members (cluster-members-str test)]
                    [])]
    (c/su
      (apply cu/start-daemon!
        {:logfile server-logfile
         :pidfile server-pidfile
         :chdir data-dir}
        server-bin
        (concat base-args init-args)))))

(defn start-gateway!
  "Starts the ormdb-gateway process on a node."
  [test node]
  (info node "Starting ormdb-gateway")
  (c/su
    (cu/start-daemon!
      {:logfile gateway-logfile
       :pidfile gateway-pidfile
       :chdir data-dir}
      gateway-bin
      :--listen (str "0.0.0.0:" gateway-port)
      :--ormdb-addr (str "127.0.0.1:" server-port))))

(defn stop-server!
  "Stops the ormdb-server process on a node."
  [test node]
  (info node "Stopping ormdb-server")
  (c/su
    (cu/stop-daemon! server-bin server-pidfile)))

(defn stop-gateway!
  "Stops the ormdb-gateway process on a node."
  [test node]
  (info node "Stopping ormdb-gateway")
  (c/su
    (cu/stop-daemon! gateway-bin gateway-pidfile)))

(defn start-ormdb!
  "Starts both ormdb-server and ormdb-gateway on a node."
  [test node]
  (start-server! test node)
  ;; Wait for server to be ready before starting gateway
  (Thread/sleep 2000)
  (start-gateway! test node)
  ;; Wait for gateway to be ready
  (Thread/sleep 1000))

(defn stop-ormdb!
  "Stops both ormdb processes on a node."
  [test node]
  (stop-gateway! test node)
  (stop-server! test node))

(defn wipe-data!
  "Removes all ormdb data from a node."
  [test node]
  (info node "Wiping ormdb data")
  (c/su
    (c/exec :rm :-rf data-dir)
    (c/exec :mkdir :-p data-dir)))

(defn kill-server!
  "Forcibly kills the ormdb-server process."
  [test node]
  (info node "Killing ormdb-server")
  (c/su
    (c/exec :pkill :-9 :-f "ormdb-server" (c/lit "|| true"))))

(defn pause-server!
  "Pauses the ormdb-server process with SIGSTOP."
  [test node]
  (info node "Pausing ormdb-server")
  (c/su
    (c/exec :pkill :-STOP :-f "ormdb-server" (c/lit "|| true"))))

(defn resume-server!
  "Resumes the ormdb-server process with SIGCONT."
  [test node]
  (info node "Resuming ormdb-server")
  (c/su
    (c/exec :pkill :-CONT :-f "ormdb-server" (c/lit "|| true"))))

(defn server-running?
  "Checks if ormdb-server is running on a node."
  [test node]
  (try
    (c/su
      (c/exec :pgrep :-f "ormdb-server")
      true)
    (catch Exception _
      false)))

(defn primaries
  "Returns the current primary node(s) in the cluster.
   In a Raft cluster, there should be exactly one primary."
  [test]
  ;; TODO: Implement actual leader detection via admin API
  ;; For now, return nil to indicate unknown
  nil)

(defn db
  "Constructs a Jepsen database implementation for ormdb."
  []
  (reify
    db/DB
    (setup! [_ test node]
      (install-ormdb! test node)
      (write-config! test node)
      (start-ormdb! test node))

    (teardown! [_ test node]
      (stop-ormdb! test node)
      (wipe-data! test node))

    db/LogFiles
    (log-files [_ test node]
      [server-logfile
       gateway-logfile
       config-file])

    db/Kill
    (start! [_ test node]
      (start-ormdb! test node))

    (kill! [_ test node]
      (kill-server! test node))

    db/Pause
    (pause! [_ test node]
      (pause-server! test node))

    (resume! [_ test node]
      (resume-server! test node))

    db/Primary
    (setup-primary! [_ test node])
    (primaries [_ test]
      (primaries test))))
