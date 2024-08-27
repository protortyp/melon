import React, { useState, useEffect } from "react";
import axios from "axios";
import JobList from "./components/JobList";
import { motion, AnimatePresence } from "framer-motion";

const App: React.FC = () => {
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [loadingDots, setLoadingDots] = useState("");
  const apiUrl = process.env.REACT_APP_API_URL || "http://[::1]:8088";

  useEffect(() => {
    const checkHealth = async () => {
      try {
        const response = await axios.get(`${apiUrl}/api/health`);
        setIsConnected(response.status === 200);
      } catch (error) {
        setIsConnected(false);
      }
    };

    checkHealth();
    const interval = setInterval(checkHealth, 1000);

    return () => clearInterval(interval);
  }, [apiUrl]);

  useEffect(() => {
    const loadingAnimation = setInterval(() => {
      setLoadingDots((prev) => {
        if (prev.length < 3) return prev + ".";
        return "";
      });
    }, 500);

    setTimeout(() => {
      clearInterval(loadingAnimation);
      setIsLoading(false);
    }, 1800);

    return () => clearInterval(loadingAnimation);
  }, []);

  return (
    <div className="min-h-screen bg-black text-green-400 flex flex-col font-mono">
      <nav className="bg-gray-900 border-b border-green-500">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex justify-between h-16">
            <div className="flex">
              <div className="flex-shrink-0 flex items-center">
                <h1 className="text-xl font-bold text-green-400">
                  &lt;/&gt; Melon Scheduler
                </h1>
              </div>
            </div>
            <div className="flex items-center">
              <div
                className={`h-3 w-3 rounded-full ${
                  isConnected ? "bg-green-500" : "bg-red-500"
                } animate-pulse`}
              ></div>
              <span className="ml-2 text-sm">
                {isConnected ? "CONNECTED" : "DISCONNECTED"}
              </span>
            </div>
          </div>
        </div>
      </nav>

      <main className="flex-grow max-w-7xl w-full mx-auto py-6 px-4 sm:px-6 lg:px-8">
        <div className="h-full border border-green-500 rounded-lg p-4 overflow-hidden">
          <AnimatePresence>
            {isLoading ? (
              <motion.div
                key="loading"
                initial={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="mb-4 text-sm"
              >
                <span className="text-green-500">&gt;</span>{" "}
                <motion.span
                  initial={{ opacity: 0 }}
                  animate={{ opacity: [0, 1, 0] }}
                  transition={{ duration: 1, repeat: Infinity }}
                >
                  Initializing job list
                </motion.span>
                <motion.span
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{
                    duration: 0.5,
                    repeat: Infinity,
                    repeatType: "reverse",
                  }}
                >
                  _
                </motion.span>
              </motion.div>
            ) : (
              <motion.div
                key="joblist"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ duration: 0.5 }}
                className="font-mono text-green-400 bg-black"
              >
                <JobList />
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </main>
    </div>
  );
};

export default App;
