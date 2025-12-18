import React, { useState } from 'react';
import { Card, Button, Badge } from '../atoms';

interface Task {
  id: string;
  name: string;
  status: 'Completed' | 'Running' | 'Failed' | 'Pending';
  progress: number;
  startTime: string;
  duration: string;
  details: string;
}

export const TaskManagerPage: React.FC = () => {
  const [tasks, setTasks] = useState<Task[]>([
    {
      id: 't1',
      name: 'VM Migration - Ubuntu-20.04',
      status: 'Running',
      progress: 65,
      startTime: '2025-12-11 10:30:00',
      duration: '3m 45s remaining',
      details: 'Migrating VM from local to network storage',
    },
    {
      id: 't2',
      name: 'Backup Completion',
      status: 'Completed',
      progress: 100,
      startTime: '2025-12-11 09:15:00',
      duration: '1h 20m',
      details: 'Full system backup completed successfully',
    },
    {
      id: 't3',
      name: 'Performance Analysis',
      status: 'Running',
      progress: 42,
      startTime: '2025-12-11 10:00:00',
      duration: '15m 30s remaining',
      details: 'Analyzing CPU and memory usage patterns',
    },
    {
      id: 't4',
      name: 'Disk Cleanup',
      status: 'Pending',
      progress: 0,
      startTime: '2025-12-11 14:00:00',
      duration: 'Scheduled',
      details: 'Will clean unused snapshots and caches',
    },
    {
      id: 't5',
      name: 'VM Snapshot Creation',
      status: 'Failed',
      progress: 78,
      startTime: '2025-12-11 08:30:00',
      duration: '15m',
      details: 'Failed due to insufficient disk space',
    },
  ]);

  const [selectedTaskId, setSelectedTaskId] = useState('t1');

  const selectedTask = tasks.find(t => t.id === selectedTaskId);

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'Completed':
        return 'bg-green-100 text-green-800';
      case 'Running':
        return 'bg-blue-100 text-blue-800';
      case 'Failed':
        return 'bg-red-100 text-red-800';
      case 'Pending':
        return 'bg-yellow-100 text-yellow-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  };

  const handleCancelTask = (taskId: string) => {
    if (window.confirm('Cancel this task?')) {
      setTasks(tasks.filter(t => t.id !== taskId));
      setSelectedTaskId(null);
    }
  };

  const handleRetryTask = (taskId: string) => {
    setTasks(tasks.map(t => 
      t.id === taskId 
        ? { ...t, status: 'Running', progress: 0, startTime: new Date().toLocaleString() }
        : t
    ));
  };

  return (
    <div className="space-y-4 p-4">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-gray-800">Task Manager</h1>
        <p className="text-gray-500">Monitor and manage background tasks</p>
      </div>

      {/* Statistics */}
      <div className="grid grid-cols-5 gap-2">
        <Card className="p-3 text-center">
          <p className="text-2xl font-bold text-gray-800">{tasks.length}</p>
          <p className="text-xs text-gray-600">Total Tasks</p>
        </Card>
        <Card className="p-3 text-center">
          <p className="text-2xl font-bold text-blue-600">{tasks.filter(t => t.status === 'Running').length}</p>
          <p className="text-xs text-gray-600">Running</p>
        </Card>
        <Card className="p-3 text-center">
          <p className="text-2xl font-bold text-green-600">{tasks.filter(t => t.status === 'Completed').length}</p>
          <p className="text-xs text-gray-600">Completed</p>
        </Card>
        <Card className="p-3 text-center">
          <p className="text-2xl font-bold text-yellow-600">{tasks.filter(t => t.status === 'Pending').length}</p>
          <p className="text-xs text-gray-600">Pending</p>
        </Card>
        <Card className="p-3 text-center">
          <p className="text-2xl font-bold text-red-600">{tasks.filter(t => t.status === 'Failed').length}</p>
          <p className="text-xs text-gray-600">Failed</p>
        </Card>
      </div>

      {/* Main Content */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Tasks List */}
        <div className="lg:col-span-2">
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">All Tasks</h2>
            <div className="space-y-2 max-h-96 overflow-y-auto">
              {tasks.map(task => (
                <div
                  key={task.id}
                  onClick={() => setSelectedTaskId(task.id)}
                  className={`p-3 rounded-lg cursor-pointer transition border-2 ${
                    selectedTaskId === task.id
                      ? 'bg-blue-100 border-blue-500'
                      : 'bg-gray-50 hover:bg-gray-100 border-gray-200'
                  }`}
                >
                  <div className="flex items-start justify-between">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <h3 className="font-semibold text-gray-800 truncate">{task.name}</h3>
                        <Badge className={`text-xs ${getStatusColor(task.status)}`}>
                          {task.status}
                        </Badge>
                      </div>
                      <div className="mt-2 w-full bg-gray-200 rounded-full h-2">
                        <div
                          className={`h-2 rounded-full transition-all ${
                            task.status === 'Completed'
                              ? 'bg-green-500'
                              : task.status === 'Running'
                              ? 'bg-blue-500'
                              : task.status === 'Failed'
                              ? 'bg-red-500'
                              : 'bg-yellow-500'
                          }`}
                          style={{ width: `${task.progress}%` }}
                        />
                      </div>
                      <p className="text-xs text-gray-500 mt-1">{task.progress}%</p>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* Task Details */}
        <div>
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Task Details</h2>
            {selectedTask ? (
              <div className="space-y-4">
                <div>
                  <p className="text-xs text-gray-500 uppercase">Name</p>
                  <p className="font-semibold text-gray-800 break-words">{selectedTask.name}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">Status</p>
                  <Badge className={`text-xs ${getStatusColor(selectedTask.status)}`}>
                    {selectedTask.status}
                  </Badge>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase mb-1">Progress</p>
                  <div className="w-full bg-gray-200 rounded-full h-3">
                    <div
                      className={`h-3 rounded-full transition-all ${
                        selectedTask.status === 'Completed'
                          ? 'bg-green-500'
                          : selectedTask.status === 'Running'
                          ? 'bg-blue-500'
                          : selectedTask.status === 'Failed'
                          ? 'bg-red-500'
                          : 'bg-yellow-500'
                      }`}
                      style={{ width: `${selectedTask.progress}%` }}
                    />
                  </div>
                  <p className="text-sm font-semibold text-gray-800 mt-1">{selectedTask.progress}%</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">Start Time</p>
                  <p className="font-semibold text-gray-800">{selectedTask.startTime}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">Duration</p>
                  <p className="font-semibold text-gray-800">{selectedTask.duration}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">Details</p>
                  <p className="text-sm text-gray-700">{selectedTask.details}</p>
                </div>

                {/* Actions */}
                <div className="pt-4 border-t space-y-2">
                  {selectedTask.status === 'Running' && (
                    <Button
                      onClick={() => handleCancelTask(selectedTask.id)}
                      className="w-full bg-red-500 hover:bg-red-600"
                    >
                      ⏹️ Cancel
                    </Button>
                  )}
                  {selectedTask.status === 'Failed' && (
                    <Button
                      onClick={() => handleRetryTask(selectedTask.id)}
                      className="w-full bg-orange-500 hover:bg-orange-600"
                    >
                      🔄 Retry
                    </Button>
                  )}
                </div>
              </div>
            ) : (
              <p className="text-gray-500 text-center py-6">Select a task to view details</p>
            )}
          </Card>
        </div>
      </div>
    </div>
  );
};
