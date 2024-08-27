import axios from "axios";
import React, { useState, useEffect, useMemo, useCallback } from "react";
import dayjs from "dayjs";
import {
  CommandLineIcon,
  ArrowPathIcon,
  ChevronUpIcon,
  ChevronDownIcon,
  XMarkIcon,
} from "@heroicons/react/24/solid";

interface RequestedResources {
  cpu_count: number;
  memory: number;
  time: number;
}

interface Job {
  id: number;
  user: string;
  script_path: string;
  script_args: string[];
  req_res: RequestedResources;
  submit_time: number;
  start_time: number | null;
  stop_time: number | null;
  status: string;
  assigned_node: string | null;
}

interface SortConfig {
  key: keyof Job;
  direction: "ascending" | "descending";
}

interface SearchConfig {
  user: string;
  status: string;
  assigned_node: string;
}

const formatTime = (timestamp: number | null) => {
  return timestamp
    ? dayjs(timestamp * 1000).format("YYYY-MM-DD HH:mm:ss")
    : "N/A";
};

const JobRow: React.FC<{ job: Job }> = ({ job }) => (
  <tr className="hover:bg-green-900 transition-colors duration-200">
    <td className="px-6 py-4 whitespace-nowrap text-sm text-green-400">
      {job.id}
    </td>
    <td className="px-6 py-4 whitespace-nowrap text-sm text-green-400">
      {job.user}
    </td>
    <td className="px-6 py-4 whitespace-nowrap text-sm text-green-400">
      {job.script_path}
    </td>
    <td className="px-6 py-4 whitespace-nowrap">
      <span
        className={`px-3 py-1 inline-flex text-xs leading-5 font-semibold rounded-full ${
          job.status === "Running"
            ? "bg-green-900 text-green-400"
            : job.status === "Completed"
            ? "bg-blue-900 text-blue-400"
            : "bg-yellow-900 text-yellow-400"
        }`}
      >
        {job.status}
      </span>
    </td>
    <td className="px-6 py-4 whitespace-nowrap text-sm text-green-400">
      {job.assigned_node || "N/A"}
    </td>
    <td className="px-6 py-4 whitespace-nowrap text-sm text-green-400">
      {formatTime(job.submit_time)}
    </td>
    <td className="px-6 py-4 whitespace-nowrap text-sm text-green-400">
      {formatTime(job.start_time)}
    </td>
    <td className="px-6 py-4 whitespace-nowrap text-sm text-green-400">
      {formatTime(job.stop_time)}
    </td>
  </tr>
);

const JobList: React.FC = () => {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const apiUrl = process.env.REACT_APP_API_URL || "http://[::1]:8088";
  const [currentPage, setCurrentPage] = useState(1);
  const [jobsPerPage, setJobsPerPage] = useState(10);
  const [sortConfig, setSortConfig] = useState<SortConfig>({
    key: "submit_time",
    direction: "descending",
  });
  const [searchConfig, setSearchConfig] = useState<SearchConfig>({
    user: "",
    status: "",
    assigned_node: "",
  });

  const fetchJobs = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await axios.get<Job[]>(`${apiUrl}/api/jobs`);
      setJobs(response.data);
    } catch (error) {
      console.error("Error fetching jobs:", error);
      setError("Failed to fetch jobs. Please try again later.");
    } finally {
      setIsLoading(false);
    }
  }, [apiUrl]);

  useEffect(() => {
    fetchJobs();
  }, []);

  const sortedJobs = useMemo(() => {
    let sortableJobs = [...jobs];
    sortableJobs.sort((a, b) => {
      const aValue = a[sortConfig.key] ?? "";
      const bValue = b[sortConfig.key] ?? "";
      if (aValue < bValue) return sortConfig.direction === "ascending" ? -1 : 1;
      if (aValue > bValue) return sortConfig.direction === "ascending" ? 1 : -1;
      return 0;
    });
    return sortableJobs;
  }, [jobs, sortConfig]);

  const filteredJobs = useMemo(() => {
    return sortedJobs.filter(
      (job) =>
        job.user.toLowerCase().includes(searchConfig.user.toLowerCase()) &&
        (searchConfig.status === "" || job.status === searchConfig.status) &&
        (job.assigned_node
          ? job.assigned_node
              .toLowerCase()
              .includes(searchConfig.assigned_node.toLowerCase())
          : searchConfig.assigned_node === "")
    );
  }, [sortedJobs, searchConfig]);

  const indexOfLastJob = currentPage * jobsPerPage;
  const indexOfFirstJob = indexOfLastJob - jobsPerPage;
  const currentJobs = filteredJobs.slice(indexOfFirstJob, indexOfLastJob);

  const paginate = (pageNumber: number) => setCurrentPage(pageNumber);

  const requestSort = (key: keyof Job) => {
    let direction: "ascending" | "descending" = "ascending";
    if (
      sortConfig &&
      sortConfig.key === key &&
      sortConfig.direction === "ascending"
    ) {
      direction = "descending";
    }
    setSortConfig({ key, direction });
  };

  const handleSearch = (key: keyof SearchConfig, value: string) => {
    setSearchConfig((prevConfig) => ({
      ...prevConfig,
      [key]: value,
    }));
    setCurrentPage(1);
  };

  const clearFilters = () => {
    setSearchConfig({
      user: "",
      status: "",
      assigned_node: "",
    });
    setCurrentPage(1);
  };

  if (isLoading) {
    return (
      <div className="container mx-auto px-4 py-8 bg-black text-green-400">
        <div className="animate-pulse">
          <div className="h-8 bg-green-900 rounded w-1/4 mb-4"></div>
          <div className="h-64 bg-green-900 rounded"></div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="container mx-auto px-4 py-8 bg-black text-red-500">
        <div
          className="bg-red-900 border border-red-500 px-4 py-3 rounded relative"
          role="alert"
        >
          <strong className="font-bold">Error!</strong>
          <span className="block sm:inline"> {error}</span>
        </div>
      </div>
    );
  }

  if (jobs.length === 0 || filteredJobs.length === 0) {
    return (
      <div className="container mx-auto px-4 py-8 text-center bg-black text-green-400">
        <CommandLineIcon className="mx-auto h-12 w-12" />
        <h3 className="mt-2 text-sm font-medium">No jobs available</h3>
        <p className="mt-1 text-sm">
          {jobs.length === 0
            ? "There are currently no jobs in the system."
            : "No jobs match the current filter criteria."}
        </p>
        <div className="mt-6 flex justify-center space-x-4">
          <button
            type="button"
            onClick={fetchJobs}
            className="inline-flex items-center px-4 py-2 border border-green-400 shadow-sm text-sm font-medium rounded-md text-green-400 bg-black hover:bg-green-900 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-400"
          >
            <ArrowPathIcon className="-ml-1 mr-2 h-5 w-5" aria-hidden="true" />
            Refresh
          </button>
          {filteredJobs.length === 0 && jobs.length > 0 && (
            <>
              <button
                type="button"
                onClick={clearFilters}
                className="inline-flex items-center px-4 py-2 border border-green-400 shadow-sm text-sm font-medium rounded-md text-green-400 bg-black hover:bg-green-900 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-400"
              >
                <XMarkIcon className="-ml-1 mr-2 h-5 w-5" aria-hidden="true" />
                Clear Filters
              </button>
              <div className="relative inline-block">
                <select
                  value={searchConfig.status}
                  onChange={(e) => handleSearch("status", e.target.value)}
                  className="appearance-none border border-green-400 rounded px-4 py-2 pr-8 text-sm bg-black text-green-400 focus:outline-none focus:ring-2 focus:ring-green-400"
                >
                  <option value="">All Statuses</option>
                  <option value="Running">Running</option>
                  <option value="Pending">Pending</option>
                  <option value="Completed">Completed</option>
                  <option value="Failed">Failed</option>
                </select>
                <div className="pointer-events-none absolute inset-y-0 right-0 flex items-center px-2 text-green-400">
                  <svg
                    className="fill-current h-4 w-4"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 20 20"
                  >
                    <path d="M9.293 12.95l.707.707L15.657 8l-1.414-1.414L10 10.828 5.757 6.586 4.343 8z" />
                  </svg>
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="container mx-auto px-4 py-8 bg-black text-green-400">
      <div className="flex justify-between items-center mb-6">
        <h2 className="text-2xl font-bold">Jobs</h2>
        <button
          onClick={fetchJobs}
          className="inline-flex items-center px-4 py-2 border border-green-400 shadow-sm text-sm font-medium rounded-md text-green-400 bg-black hover:bg-green-900 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-400"
        >
          <ArrowPathIcon className="-ml-1 mr-2 h-5 w-5" aria-hidden="true" />
          Refresh
        </button>
      </div>
      <div className="mb-4 flex justify-between items-center">
        <div>
          <label htmlFor="jobsPerPage" className="mr-2">
            Show:
          </label>
          <select
            id="jobsPerPage"
            value={jobsPerPage}
            onChange={(e) => setJobsPerPage(Number(e.target.value))}
            className="border border-green-400 rounded px-2 py-1 bg-black text-green-400"
          >
            <option value={10}>10</option>
            <option value={50}>50</option>
            <option value={100}>100</option>
          </select>
        </div>
      </div>
      <div className="overflow-x-auto bg-black rounded-lg shadow border border-green-400">
        <table className="w-full table-auto">
          <thead>
            <tr className="bg-green-900 border-b border-green-400">
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                ID
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                <div className="flex items-center">
                  <span
                    className="cursor-pointer mr-2"
                    onClick={() => requestSort("user")}
                  >
                    User
                    {sortConfig.key === "user" &&
                      (sortConfig.direction === "ascending" ? (
                        <ChevronUpIcon className="inline w-4 h-4 ml-1" />
                      ) : (
                        <ChevronDownIcon className="inline w-4 h-4 ml-1" />
                      ))}
                  </span>
                </div>
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                Script Path
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                <div className="flex items-center">
                  <span
                    className="cursor-pointer mr-2"
                    onClick={() => requestSort("status")}
                  >
                    Status
                    {sortConfig.key === "status" &&
                      (sortConfig.direction === "ascending" ? (
                        <ChevronUpIcon className="inline w-4 h-4 ml-1" />
                      ) : (
                        <ChevronDownIcon className="inline w-4 h-4 ml-1" />
                      ))}
                  </span>
                  <select
                    value={searchConfig.status}
                    onChange={(e) => handleSearch("status", e.target.value)}
                    className="border border-green-400 rounded px-2 py-1 text-sm bg-black text-green-400"
                  >
                    <option value="">All</option>
                    <option value="Running">Running</option>
                    <option value="Pending">Pending</option>
                    <option value="Completed">Completed</option>
                    <option value="Failed">Failed</option>
                  </select>
                </div>
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                <div className="flex items-center">
                  <span
                    className="cursor-pointer mr-2"
                    onClick={() => requestSort("assigned_node")}
                  >
                    Assigned Node
                    {sortConfig.key === "assigned_node" &&
                      (sortConfig.direction === "ascending" ? (
                        <ChevronUpIcon className="inline w-4 h-4 ml-1" />
                      ) : (
                        <ChevronDownIcon className="inline w-4 h-4 ml-1" />
                      ))}
                  </span>
                </div>
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                Submitted
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                Started
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider">
                Finished
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-green-900">
            {currentJobs.map((job) => (
              <JobRow key={job.id} job={job} />
            ))}
          </tbody>
        </table>
      </div>
      <div className="mt-4 flex justify-between items-center">
        <button
          onClick={() => paginate(currentPage - 1)}
          disabled={currentPage === 1}
          className="px-4 py-2 border border-green-400 rounded text-sm font-medium bg-black text-green-400 disabled:opacity-50 hover:bg-green-900"
        >
          Previous
        </button>
        <span>
          Page {currentPage} of {Math.ceil(filteredJobs.length / jobsPerPage)}
        </span>
        <button
          onClick={() => paginate(currentPage + 1)}
          disabled={indexOfLastJob >= filteredJobs.length}
          className="px-4 py-2 border border-green-400 rounded text-sm font-medium bg-black text-green-400 disabled:opacity-50 hover:bg-green-900"
        >
          Next
        </button>
      </div>
    </div>
  );
};

export default JobList;
