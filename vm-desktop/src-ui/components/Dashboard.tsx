import React from 'react';

interface VM {
  id: string;
  name: string;
  state: string;
  cpu_count: number;
  memory_mb: number;
  display_mode: string;
}

interface DashboardProps {
  vms: VM[];
}

export default function Dashboard({ vms }: DashboardProps) {
  const runningVms = vms.filter(
    (vm) => typeof vm.state === 'string' && vm.state === 'Running'
  ).length;

  const totalCpu = vms.reduce((sum, vm) => sum + vm.cpu_count, 0);
  const totalMemory = vms.reduce((sum, vm) => sum + vm.memory_mb, 0);

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      {/* Total VMs */}
      <div className="bg-white rounded-lg shadow p-6">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-gray-500 text-sm">Total VMs</p>
            <p className="text-3xl font-bold text-gray-800">{vms.length}</p>
          </div>
          <div className="text-4xl text-blue-500">🖥️</div>
        </div>
      </div>

      {/* Running VMs */}
      <div className="bg-white rounded-lg shadow p-6">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-gray-500 text-sm">Running</p>
            <p className="text-3xl font-bold text-green-600">{runningVms}</p>
          </div>
          <div className="text-4xl text-green-500">▶️</div>
        </div>
      </div>

      {/* Total CPU Cores */}
      <div className="bg-white rounded-lg shadow p-6">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-gray-500 text-sm">Total CPU Cores</p>
            <p className="text-3xl font-bold text-yellow-600">{totalCpu}</p>
          </div>
          <div className="text-4xl text-yellow-500">⚙️</div>
        </div>
      </div>

      {/* Total Memory */}
      <div className="bg-white rounded-lg shadow p-6">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-gray-500 text-sm">Total Memory</p>
            <p className="text-3xl font-bold text-purple-600">
              {(totalMemory / 1024).toFixed(1)} GB
            </p>
          </div>
          <div className="text-4xl text-purple-500">💾</div>
        </div>
      </div>

      {/* VM Details */}
      <div className="col-span-full bg-white rounded-lg shadow p-6">
        <h3 className="text-lg font-semibold text-gray-800 mb-4">
          VM Details
        </h3>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="border-b border-gray-200">
              <tr className="text-left">
                <th className="pb-2 text-gray-600">Name</th>
                <th className="pb-2 text-gray-600">Status</th>
                <th className="pb-2 text-gray-600">CPU</th>
                <th className="pb-2 text-gray-600">Memory</th>
                <th className="pb-2 text-gray-600">Display Mode</th>
              </tr>
            </thead>
            <tbody>
              {vms.map((vm) => (
                <tr key={vm.id} className="border-b border-gray-100 hover:bg-gray-50">
                  <td className="py-3">{vm.name}</td>
                  <td className="py-3">
                    <span
                      className={`px-2 py-1 rounded text-xs font-medium ${
                        typeof vm.state === 'string' &&
                        vm.state === 'Running'
                          ? 'bg-green-100 text-green-800'
                          : typeof vm.state === 'string' &&
                            vm.state === 'Paused'
                            ? 'bg-yellow-100 text-yellow-800'
                            : 'bg-red-100 text-red-800'
                      }`}
                    >
                      {typeof vm.state === 'string'
                        ? vm.state
                        : 'Error'}
                    </span>
                  </td>
                  <td className="py-3">{vm.cpu_count}</td>
                  <td className="py-3">{vm.memory_mb} MB</td>
                  <td className="py-3">{vm.display_mode}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
