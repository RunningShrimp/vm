import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer, AreaChart, Area } from 'recharts';

interface PerformanceMetrics {
  id: string;
  cpu_usage: number;
  memory_usage_mb: number;
  disk_io_read_mb_s: number;
  disk_io_write_mb_s: number;
  network_rx_mb_s: number;
  network_tx_mb_s: number;
  uptime_secs: number;
  jit_compilation_rate?: number;
  tlb_hit_rate?: number;
  cache_hit_rate?: number;
  instruction_count?: number;
  syscalls_per_sec?: number;
  page_faults_per_sec?: number;
  context_switches_per_sec?: number;
}

interface PerformanceDashboardProps {
  vmId: string;
  vmName: string;
}

export const PerformanceDashboard: React.FC<PerformanceDashboardProps> = ({ vmId, vmName }) => {
  const [metrics, setMetrics] = useState<PerformanceMetrics | null>(null);
  const [historicalData, setHistoricalData] = useState<PerformanceMetrics[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedTimeRange, setSelectedTimeRange] = useState<'1m' | '5m' | '15m' | '1h' | 'all'>('5m');
  const [refreshInterval, setRefreshInterval] = useState(1000); // ms

  useEffect(() => {
    const fetchMetrics = async () => {
      try {
        const metricsData = await invoke<PerformanceMetrics>('get_vm_metrics', { id: vmId });
        setMetrics(metricsData);
        
        // Add to historical data
        setHistoricalData(prev => {
          const newData = [...prev, { ...metricsData }];
          // Keep only last 100 data points
          return newData.slice(-100);
        });
        
        setError(null);
      } catch (err) {
        setError(`Failed to fetch metrics: ${err}`);
      } finally {
        setIsLoading(false);
      }
    };

    // Initial fetch
    fetchMetrics();

    // Set up interval for real-time updates
    const interval = setInterval(fetchMetrics, refreshInterval);

    return () => clearInterval(interval);
  }, [vmId, refreshInterval]);

  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatDuration = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    
    if (hours > 0) {
      return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
      return `${minutes}m ${secs}s`;
    } else {
      return `${secs}s`;
    }
  };

  const getFilteredHistoricalData = () => {
    if (selectedTimeRange === 'all') return historicalData;
    
    const now = Date.now();
    const timeRanges = {
      '1m': 60 * 1000,
      '5m': 5 * 60 * 1000,
      '15m': 15 * 60 * 1000,
      '1h': 60 * 60 * 1000,
    };
    
    const cutoff = now - timeRanges[selectedTimeRange];
    return historicalData.filter((_, index) => {
      // Approximate timestamp based on index (1 second intervals)
      const timestamp = now - (historicalData.length - index) * 1000;
      return timestamp >= cutoff;
    });
  };

  if (isLoading && !metrics) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-md p-4">
        <div className="text-red-800">{error}</div>
      </div>
    );
  }

  if (!metrics) {
    return (
      <div className="bg-gray-50 border border-gray-200 rounded-md p-4">
        <div className="text-gray-800">No metrics available for VM: {vmName}</div>
      </div>
    );
  }

  const filteredData = getFilteredHistoricalData();

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <h2 className="text-2xl font-bold text-gray-900">Performance Dashboard - {vmName}</h2>
        <div className="flex space-x-2">
          <select
            value={selectedTimeRange}
            onChange={(e) => setSelectedTimeRange(e.target.value as any)}
            className="border border-gray-300 rounded-md px-3 py-2 text-sm"
          >
            <option value="1m">Last 1 min</option>
            <option value="5m">Last 5 min</option>
            <option value="15m">Last 15 min</option>
            <option value="1h">Last 1 hour</option>
            <option value="all">All</option>
          </select>
          <select
            value={refreshInterval}
            onChange={(e) => setRefreshInterval(Number(e.target.value))}
            className="border border-gray-300 rounded-md px-3 py-2 text-sm"
          >
            <option value={500}>0.5s</option>
            <option value={1000}>1s</option>
            <option value={2000}>2s</option>
            <option value={5000}>5s</option>
          </select>
        </div>
      </div>

      {/* Key Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="bg-white p-6 rounded-lg shadow">
          <div className="text-sm font-medium text-gray-500">CPU Usage</div>
          <div className="mt-2 text-3xl font-semibold text-gray-900">
            {metrics.cpu_usage.toFixed(1)}%
          </div>
          <div className="mt-2 flex items-center text-sm">
            <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
              metrics.cpu_usage > 80 ? 'bg-red-100 text-red-800' :
              metrics.cpu_usage > 60 ? 'bg-yellow-100 text-yellow-800' :
              'bg-green-100 text-green-800'
            }`}>
              {metrics.cpu_usage > 80 ? 'High' : metrics.cpu_usage > 60 ? 'Medium' : 'Low'}
            </span>
          </div>
        </div>

        <div className="bg-white p-6 rounded-lg shadow">
          <div className="text-sm font-medium text-gray-500">Memory Usage</div>
          <div className="mt-2 text-3xl font-semibold text-gray-900">
            {formatBytes(metrics.memory_usage_mb * 1024 * 1024)}
          </div>
          <div className="mt-2 text-sm text-gray-500">
            Uptime: {formatDuration(metrics.uptime_secs)}
          </div>
        </div>

        <div className="bg-white p-6 rounded-lg shadow">
          <div className="text-sm font-medium text-gray-500">Disk I/O</div>
          <div className="mt-2 text-3xl font-semibold text-gray-900">
            {(metrics.disk_io_read_mb_s + metrics.disk_io_write_mb_s).toFixed(1)} MB/s
          </div>
          <div className="mt-2 text-sm text-gray-500">
            Read: {metrics.disk_io_read_mb_s.toFixed(1)} MB/s, Write: {metrics.disk_io_write_mb_s.toFixed(1)} MB/s
          </div>
        </div>

        <div className="bg-white p-6 rounded-lg shadow">
          <div className="text-sm font-medium text-gray-500">Network I/O</div>
          <div className="mt-2 text-3xl font-semibold text-gray-900">
            {(metrics.network_rx_mb_s + metrics.network_tx_mb_s).toFixed(1)} MB/s
          </div>
          <div className="mt-2 text-sm text-gray-500">
            RX: {metrics.network_rx_mb_s.toFixed(1)} MB/s, TX: {metrics.network_tx_mb_s.toFixed(1)} MB/s
          </div>
        </div>
      </div>

      {/* Enhanced Metrics (if available) */}
      {metrics.jit_compilation_rate !== undefined && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div className="bg-white p-6 rounded-lg shadow">
            <div className="text-sm font-medium text-gray-500">JIT Compilation Rate</div>
            <div className="mt-2 text-3xl font-semibold text-gray-900">
              {metrics.jit_compilation_rate?.toFixed(0) || 0}
            </div>
            <div className="mt-2 text-sm text-gray-500">compilations/sec</div>
          </div>

          <div className="bg-white p-6 rounded-lg shadow">
            <div className="text-sm font-medium text-gray-500">TLB Hit Rate</div>
            <div className="mt-2 text-3xl font-semibold text-gray-900">
              {metrics.tlb_hit_rate?.toFixed(1) || 0}%
            </div>
            <div className="mt-2 text-sm text-gray-500">Translation Lookaside Buffer</div>
          </div>

          <div className="bg-white p-6 rounded-lg shadow">
            <div className="text-sm font-medium text-gray-500">Cache Hit Rate</div>
            <div className="mt-2 text-3xl font-semibold text-gray-900">
              {metrics.cache_hit_rate?.toFixed(1) || 0}%
            </div>
            <div className="mt-2 text-sm text-gray-500">Instruction Cache</div>
          </div>

          <div className="bg-white p-6 rounded-lg shadow">
            <div className="text-sm font-medium text-gray-500">Instructions Executed</div>
            <div className="mt-2 text-3xl font-semibold text-gray-900">
              {(metrics.instruction_count || 0).toLocaleString()}
            </div>
            <div className="mt-2 text-sm text-gray-500">Total count</div>
          </div>
        </div>
      )}

      {/* Charts */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* CPU Usage Chart */}
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-lg font-medium text-gray-900 mb-4">CPU Usage Over Time</h3>
          <ResponsiveContainer width="100%" height={300}>
            <AreaChart data={filteredData}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis 
                dataKey="uptime_secs" 
                tickFormatter={(value) => formatDuration(value)}
              />
              <YAxis domain={[0, 100]} />
              <Tooltip 
                formatter={(value: any) => [`${value}%`, 'CPU Usage']}
                labelFormatter={(value) => `Time: ${formatDuration(value)}`}
              />
              <Area 
                type="monotone" 
                dataKey="cpu_usage" 
                stroke="#3B82F6" 
                fill="#93BBFC" 
                strokeWidth={2}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Memory Usage Chart */}
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-lg font-medium text-gray-900 mb-4">Memory Usage Over Time</h3>
          <ResponsiveContainer width="100%" height={300}>
            <LineChart data={filteredData}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis 
                dataKey="uptime_secs" 
                tickFormatter={(value) => formatDuration(value)}
              />
              <YAxis tickFormatter={(value) => formatBytes(value * 1024 * 1024)} />
              <Tooltip 
                formatter={(value: any) => [formatBytes(value * 1024 * 1024), 'Memory Usage']}
                labelFormatter={(value) => `Time: ${formatDuration(value)}`}
              />
              <Line 
                type="monotone" 
                dataKey="memory_usage_mb" 
                stroke="#10B981" 
                strokeWidth={2}
                dot={false}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Disk I/O Chart */}
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-lg font-medium text-gray-900 mb-4">Disk I/O Over Time</h3>
          <ResponsiveContainer width="100%" height={300}>
            <LineChart data={filteredData}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis 
                dataKey="uptime_secs" 
                tickFormatter={(value) => formatDuration(value)}
              />
              <YAxis />
              <Tooltip 
                formatter={(value: any, name: any) => [
                  `${value} MB/s`, 
                  name === 'disk_io_read_mb_s' ? 'Disk Read' : 'Disk Write'
                ]}
                labelFormatter={(value) => `Time: ${formatDuration(value)}`}
              />
              <Legend />
              <Line 
                type="monotone" 
                dataKey="disk_io_read_mb_s" 
                stroke="#F59E0B" 
                strokeWidth={2}
                dot={false}
                name="Disk Read"
              />
              <Line 
                type="monotone" 
                dataKey="disk_io_write_mb_s" 
                stroke="#EF4444" 
                strokeWidth={2}
                dot={false}
                name="Disk Write"
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Network I/O Chart */}
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-lg font-medium text-gray-900 mb-4">Network I/O Over Time</h3>
          <ResponsiveContainer width="100%" height={300}>
            <LineChart data={filteredData}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis 
                dataKey="uptime_secs" 
                tickFormatter={(value) => formatDuration(value)}
              />
              <YAxis />
              <Tooltip 
                formatter={(value: any, name: any) => [
                  `${value} MB/s`, 
                  name === 'network_rx_mb_s' ? 'Network RX' : 'Network TX'
                ]}
                labelFormatter={(value) => `Time: ${formatDuration(value)}`}
              />
              <Legend />
              <Line 
                type="monotone" 
                dataKey="network_rx_mb_s" 
                stroke="#8B5CF6" 
                strokeWidth={2}
                dot={false}
                name="Network RX"
              />
              <Line 
                type="monotone" 
                dataKey="network_tx_mb_s" 
                stroke="#EC4899" 
                strokeWidth={2}
                dot={false}
                name="Network TX"
              />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  );
};