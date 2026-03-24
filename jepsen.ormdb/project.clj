(defproject jepsen.ormdb "0.1.0-SNAPSHOT"
  :description "Jepsen tests for ORMDB - an ORM-first relational database"
  :url "https://github.com/Skelf-Research/ormdb"
  :license {:name "MIT"
            :url "https://opensource.org/licenses/MIT"}

  :dependencies [[org.clojure/clojure "1.11.1"]
                 [jepsen "0.3.5"]
                 [clj-http "3.12.3"]
                 [cheshire "5.12.0"]]

  :main jepsen.ormdb.core

  :jvm-opts ["-Xmx8g"
             "-Djava.awt.headless=true"
             "-server"]

  :repl-options {:init-ns jepsen.ormdb.core}

  :profiles {:dev {:dependencies [[org.clojure/tools.namespace "1.4.4"]]
                   :source-paths ["dev"]}
             :uberjar {:aot :all
                       :jvm-opts ["-Dclojure.compiler.direct-linking=true"]}})
