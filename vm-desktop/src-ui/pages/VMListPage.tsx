import React, { useState } from 'react';
import { Button, Input, Badge, Tabs } from '../atoms';
import { VMCard, VMToolbar } from '../molecules';
import { GridLayout, CardLayout } from '../templates';

interface VM {
  id: string;
  name: string;
  status: 'running' | 'stopped' | 'paused' | 'suspended';
  cpu_count: number;
  memory_mb: number;
  disk_gb: number;
  uptime_seconds?: number;
  disk_used_gb?: number;
}

interface VMListPageProps {
  vms: VM[];
  loading?: boolean;
  onVMClick?: (id: string) => void;
  onCreateVM?: () => void;
  onRefresh?: () => void;
  onStart?: (id: string) => void;
  onStop?: (id: string) => void;
  onPause?: (id: string) => void;
  onResume?: (id: string) => void;
  onDelete?: (id: string) => void;
}

export const VMListPage: React.FC<VMListPageProps> = ({
  vms,
  loading = false,
  onVMClick,
  onCreateVM,
  onRefresh,
  onStart,
  onStop,
  onPause,
  onResume,
  onDelete,
}) => {
  const [viewMode, setViewMode] = useState<'grid' | 'list' | 'compact'>('grid');
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<'all' | 'running' | 'stopped'>('all');

  const filteredVMs = vms.filter(vm => {
    const matchesSearch = vm.name.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesFilter = filterStatus === 'all' || vm.status === filterStatus;
    return matchesSearch && matchesFilter;
  });

  const runningCount = vms.filter(vm => vm.status === 'running').length;
  const stoppedCount = vms.filter(vm => vm.status === 'stopped').length;

  return (
    <div className="space-y-6">
      {/* 搜索和过滤 */}
      <div className="flex gap-4">
        <div className="flex-1">
          <Input
            placeholder="搜索虚拟机..."
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
          />
        </div>
        <div className="flex gap-2">
          <Button
            variant={filterStatus === 'all' ? 'primary' : 'outline'}
            onClick={() => setFilterStatus('all')}
          >
            全部 ({vms.length})
          </Button>
          <Button
            variant={filterStatus === 'running' ? 'primary' : 'outline'}
            onClick={() => setFilterStatus('running')}
          >
            运行中 ({runningCount})
          </Button>
          <Button
            variant={filterStatus === 'stopped' ? 'primary' : 'outline'}
            onClick={() => setFilterStatus('stopped')}
          >
            已停止 ({stoppedCount})
          </Button>
        </div>
      </div>

      {/* 工具栏 */}
      <div className="flex justify-between items-center">
        <div className="flex gap-2">
          <Button
            variant={viewMode === 'grid' ? 'primary' : 'outline'}
            onClick={() => setViewMode('grid')}
          >
            📊 网格
          </Button>
          <Button
            variant={viewMode === 'list' ? 'primary' : 'outline'}
            onClick={() => setViewMode('list')}
          >
            📋 列表
          </Button>
          <Button
            variant={viewMode === 'compact' ? 'primary' : 'outline'}
            onClick={() => setViewMode('compact')}
          >
            📝 紧凑
          </Button>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onRefresh}>🔄 刷新</Button>
          <Button variant="primary" onClick={onCreateVM}>➕ 新建</Button>
        </div>
      </div>

      {/* VM 列表 - 网格视图 */}
      {viewMode === 'grid' && (
        <GridLayout columns={3} gap="md">
          {filteredVMs.map(vm => (
            <VMCard
              key={vm.id}
              id={vm.id}
              name={vm.name}
              status={vm.status}
              cpu_count={vm.cpu_count}
              memory_mb={vm.memory_mb}
              disk_gb={vm.disk_gb}
              onClick={() => onVMClick?.(vm.id)}
            />
          ))}
        </GridLayout>
      )}

      {/* VM 列表 - 列表视图 */}
      {viewMode === 'list' && (
        <div className="space-y-2">
          {filteredVMs.map(vm => (
            <div
              key={vm.id}
              className="flex items-center justify-between p-4 bg-white rounded-lg border border-gray-200 hover:shadow-md transition-shadow cursor-pointer"
              onClick={() => onVMClick?.(vm.id)}
            >
              <div className="flex-1">
                <div className="flex items-center gap-3">
                  <span className="text-xl">🖥️</span>
                  <div>
                    <div className="font-medium text-gray-900">{vm.name}</div>
                    <div className="text-sm text-gray-600">
                      CPU: {vm.cpu_count} | 内存: {vm.memory_mb}MB | 磁盘: {vm.disk_gb}GB
                    </div>
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-3">
                {vm.status === 'running' && (
                  <Badge variant="success">运行中</Badge>
                )}
                {vm.status === 'stopped' && (
                  <Badge variant="default">已停止</Badge>
                )}
                {vm.status === 'paused' && (
                  <Badge variant="warning">暂停</Badge>
                )}
                <div className="flex gap-2">
                  {vm.status === 'stopped' && (
                    <Button size="sm" variant="success" onClick={(e) => {
                      e.stopPropagation();
                      onStart?.(vm.id);
                    }}>
                      ▶️
                    </Button>
                  )}
                  {vm.status === 'running' && (
                    <>
                      <Button size="sm" variant="warning" onClick={(e) => {
                        e.stopPropagation();
                        onPause?.(vm.id);
                      }}>
                        ⏸️
                      </Button>
                      <Button size="sm" variant="danger" onClick={(e) => {
                        e.stopPropagation();
                        onStop?.(vm.id);
                      }}>
                        ⏹️
                      </Button>
                    </>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* VM 列表 - 紧凑视图 */}
      {viewMode === 'compact' && (
        <div className="bg-white rounded-lg border border-gray-200 overflow-hidden">
          <table className="w-full">
            <thead>
              <tr className="border-b border-gray-200 bg-gray-50">
                <th className="text-left px-6 py-3 text-sm font-semibold text-gray-900">虚拟机</th>
                <th className="text-left px-6 py-3 text-sm font-semibold text-gray-900">状态</th>
                <th className="text-left px-6 py-3 text-sm font-semibold text-gray-900">CPU</th>
                <th className="text-left px-6 py-3 text-sm font-semibold text-gray-900">内存</th>
                <th className="text-left px-6 py-3 text-sm font-semibold text-gray-900">磁盘</th>
                <th className="text-left px-6 py-3 text-sm font-semibold text-gray-900">操作</th>
              </tr>
            </thead>
            <tbody>
              {filteredVMs.map(vm => (
                <tr
                  key={vm.id}
                  className="border-b border-gray-200 hover:bg-gray-50 cursor-pointer"
                  onClick={() => onVMClick?.(vm.id)}
                >
                  <td className="px-6 py-3 text-sm text-gray-900 font-medium">{vm.name}</td>
                  <td className="px-6 py-3 text-sm">
                    {vm.status === 'running' && (
                      <Badge variant="success">运行中</Badge>
                    )}
                    {vm.status === 'stopped' && (
                      <Badge variant="default">已停止</Badge>
                    )}
                    {vm.status === 'paused' && (
                      <Badge variant="warning">暂停</Badge>
                    )}
                  </td>
                  <td className="px-6 py-3 text-sm text-gray-600">{vm.cpu_count} 核</td>
                  <td className="px-6 py-3 text-sm text-gray-600">{vm.memory_mb} MB</td>
                  <td className="px-6 py-3 text-sm text-gray-600">{vm.disk_gb} GB</td>
                  <td className="px-6 py-3 text-sm">
                    <div className="flex gap-2" onClick={e => e.stopPropagation()}>
                      {vm.status === 'stopped' && (
                        <Button size="sm" variant="success" onClick={() => onStart?.(vm.id)}>
                          启动
                        </Button>
                      )}
                      {vm.status === 'running' && (
                        <Button size="sm" variant="danger" onClick={() => onStop?.(vm.id)}>
                          停止
                        </Button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* 空状态 */}
      {filteredVMs.length === 0 && (
        <div className="flex flex-col items-center justify-center py-12 bg-white rounded-lg border border-gray-200">
          <span className="text-5xl mb-4">📭</span>
          <h3 className="text-lg font-semibold text-gray-900 mb-2">未找到虚拟机</h3>
          <p className="text-gray-600 mb-6">
            {searchQuery ? '没有匹配搜索条件的虚拟机' : '还没有虚拟机，创建一个开始吧'}
          </p>
          {!searchQuery && (
            <Button variant="primary" onClick={onCreateVM}>
              创建虚拟机
            </Button>
          )}
        </div>
      )}
    </div>
  );
};
