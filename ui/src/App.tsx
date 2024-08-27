import React, { useState, useEffect } from "react";
import axios from "axios";
import JobList from "./components/JobList";

const App: React.FC = () => {
  const [isConnected, setIsConnected] = useState(false);
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
        <div className="h-full border border-green-500 rounded-lg p-4">
          <div className="mb-4 text-sm">
            <span className="text-green-500">&gt;</span> Initializing job
            list...
          </div>
          <JobList />
        </div>
      </main>
    </div>
  );
};

export default App;
