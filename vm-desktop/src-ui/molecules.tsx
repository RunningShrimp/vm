/**
 * 分子组件库 - 由原子组件组合而成
 * 模仿 VMware Workstation 的常见模块
 */

import React from 'react';
import { Button, Badge, Card, Input, Progress, Spinner, Tooltip } from './atoms';

/** VM 卡片 - 显示单个虚拟机 */
export interface VMCardProps {
  id: string;
  name: string;
  status: 'running' | 'stopped' | 'paused' | 'error';
  cpu: number;
  memory: number;
  displayMode: 'GUI' | 'Terminal';
  onStart?: () => void;
  onStop?: () => void;
  onPause?: () => void;
  onClick?: () => void;
}

export const VMCard: React.FC<VMCardProps> = ({
  name,
  status,
  cpu,
  memory,
  displayMode,
  onStart,
  onStop,
  onPause,
  onClick,
}) => {
  const statusConfig = {
    running: { color: 'success', label: '运行中', icon: '▶' },
    stopped: { color: 'danger', label: '已停止', icon: '⏹' },
    paused: { color: 'warning', label: '暂停中', icon: '⏸' },
    error: { color: 'danger', label: '错误', icon: '⚠' },
  };

  const config = statusConfig[status];

  return (
    <Card className="cursor-pointer hover:shadow-lg transition" onClick={onClick}>
      <div className="flex justify-between items-start mb-4">
        <div className="flex-1">
          <h3 className="text-lg font-semibold text-gray-900">{name}</h3>
          <Badge variant={config.color as any}>
            {config.icon} {config.label}
          </Badge>
        </div>
      </div>

      <div className="space-y-2 mb-4">
        <div className="flex justify-between text-sm">
          <span className="text-gray-600">CPU</span>
          <span className="font-medium">{cpu} 核心</span>
        </div>
        <div className="flex justify-between text-sm">
          <span className="text-gray-600">内存</span>
          <span className="font-medium">{memory} MB</span>
        </div>
        <div className="flex justify-between text-sm">
          <span className="text-gray-600">显示模式</span>
          <span className="font-medium">{displayMode}</span>
        </div>
      </div>

      <div className="flex gap-2 pt-4 border-t border-gray-200">
        {status === 'stopped' && (
          <Button variant="success" size="sm" onClick={(e) => { e.stopPropagation(); onStart?.(); }} className="flex-1">
            启动
          </Button>
        )}
        {status === 'running' && (
          <>
            <Button variant="warning" size="sm" onClick={(e) => { e.stopPropagation(); onPause?.(); }} className="flex-1">
              暂停
            </Button>
            <Button variant="danger" size="sm" onClick={(e) => { e.stopPropagation(); onStop?.(); }} className="flex-1">
              停止
            </Button>
          </>
        )}
        {status === 'paused' && (
          <Button variant="success" size="sm" onClick={(e) => { e.stopPropagation(); onStart?.(); }} className="flex-1">
            恢复
          </Button>
        )}
      </div>
    </Card>
  );
};

/** 性能指标卡 */
export interface MetricsCardProps {
  title: string;
  value: string | number;
  unit?: string;
  icon?: string;
  trend?: 'up' | 'down' | 'stable';
}

export const MetricsCard: React.FC<MetricsCardProps> = ({ title, value, unit, icon, trend }) => {
  const trendIcon = {
    up: '📈',
    down: '📉',
    stable: '➡️',
  };

  return (
    <Card className="text-center">
      {icon && <div className="text-4xl mb-2">{icon}</div>}
      <p className="text-gray-600 text-sm">{title}</p>
      <p className="text-3xl font-bold text-gray-900 my-2">
        {value}
        {unit && <span className="text-lg text-gray-600"> {unit}</span>}
      </p>
      {trend && (
        <div className="text-xl">{trendIcon[trend]}</div>
      )}
    </Card>
  );
};

/** 系统资源监控条 */
export interface ResourceBarProps {
  label: string;
  used: number;
  total: number;
  unit?: string;
}

export const ResourceBar: React.FC<ResourceBarProps> = ({ label, used, total, unit = 'MB' }) => {
  const percentage = (used / total) * 100;
  const variant = percentage > 80 ? 'danger' : percentage > 60 ? 'warning' : 'success';

  return (
    <div className="mb-4">
      <div className="flex justify-between text-sm mb-2">
        <span className="font-medium text-gray-700">{label}</span>
        <span className="text-gray-600">
          {used} / {total} {unit}
        </span>
      </div>
      <Progress value={used} max={total} variant={variant as any} />
    </div>
  );
};

/** 创建 VM 表单 */
export interface CreateVMFormProps {
  onSubmit?: (data: any) => void;
  onCancel?: () => void;
  loading?: boolean;
}

export const CreateVMForm: React.FC<CreateVMFormProps> = ({ onSubmit, onCancel, loading = false }) => {
  const [formData, setFormData] = React.useState({
    name: '',
    cpu: 2,
    memory: 2048,
    displayMode: 'GUI',
    osType: 'Ubuntu',
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit?.(formData);
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <Input
        label="虚拟机名称"
        placeholder="输入虚拟机名称"
        value={formData.name}
        onChange={(e) => setFormData({ ...formData, name: e.target.value })}
        required
      />

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">CPU 核心数</label>
          <input
            type="number"
            min="1"
            max="16"
            value={formData.cpu}
            onChange={(e) => setFormData({ ...formData, cpu: parseInt(e.target.value) })}
            className="w-full px-3 py-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">内存 (MB)</label>
          <input
            type="number"
            min="512"
            step="512"
            value={formData.memory}
            onChange={(e) => setFormData({ ...formData, memory: parseInt(e.target.value) })}
            className="w-full px-3 py-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500"
          />
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">显示模式</label>
          <select
            value={formData.displayMode}
            onChange={(e) => setFormData({ ...formData, displayMode: e.target.value })}
            className="w-full px-3 py-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500"
          >
            <option>GUI</option>
            <option>Terminal</option>
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">操作系统</label>
          <select
            value={formData.osType}
            onChange={(e) => setFormData({ ...formData, osType: e.target.value })}
            className="w-full px-3 py-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500"
          >
            <option>Ubuntu</option>
            <option>Debian</option>
            <option>Windows</option>
            <option>CentOS</option>
          </select>
        </div>
      </div>

      <div className="flex gap-2 pt-4">
        <Button type="submit" variant="primary" disabled={loading} className="flex-1">
          {loading ? <Spinner size="sm" /> : '创建'}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel} className="flex-1">
          取消
        </Button>
      </div>
    </form>
  );
};

/** VM 操作工具栏 */
export interface VMToolbarProps {
  onNew?: () => void;
  onRefresh?: () => void;
  onSettings?: () => void;
  selectedCount?: number;
}

export const VMToolbar: React.FC<VMToolbarProps> = ({ onNew, onRefresh, onSettings, selectedCount }) => {
  return (
    <div className="bg-gray-50 border-b border-gray-200 px-6 py-3 flex items-center justify-between">
      <div className="flex gap-2">
        <Tooltip content="创建新虚拟机">
          <Button variant="primary" size="sm" onClick={onNew}>
            ➕ 新建
          </Button>
        </Tooltip>
        <Tooltip content="刷新列表">
          <Button variant="outline" size="sm" onClick={onRefresh}>
            🔄 刷新
          </Button>
        </Tooltip>
        <Tooltip content="设置">
          <Button variant="outline" size="sm" onClick={onSettings}>
            ⚙️ 设置
          </Button>
        </Tooltip>
      </div>
      {selectedCount !== undefined && selectedCount > 0 && (
        <span className="text-sm text-gray-600">
          已选择 {selectedCount} 个虚拟机
        </span>
      )}
    </div>
  );
};

/** 加载状态提示 */
export const LoadingPlaceholder: React.FC<{ message?: string }> = ({ message = '加载中...' }) => (
  <div className="flex flex-col items-center justify-center py-12">
    <Spinner size="lg" />
    <p className="text-gray-600 mt-4">{message}</p>
  </div>
);

/** 空状态提示 */
export const EmptyState: React.FC<{ icon?: string; title: string; description?: string; action?: React.ReactNode }> = ({
  icon = '📭',
  title,
  description,
  action,
}) => (
  <div className="flex flex-col items-center justify-center py-12">
    <div className="text-6xl mb-4">{icon}</div>
    <h3 className="text-lg font-semibold text-gray-900 mb-2">{title}</h3>
    {description && <p className="text-gray-600 mb-4">{description}</p>}
    {action}
  </div>
);

/** 确认对话框 */
export interface ConfirmDialogProps {
  title: string;
  message: string;
  onConfirm?: () => void;
  onCancel?: () => void;
  confirmText?: string;
  cancelText?: string;
  variant?: 'primary' | 'danger';
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  title,
  message,
  onConfirm,
  onCancel,
  confirmText = '确认',
  cancelText = '取消',
  variant = 'primary',
}) => (
  <div className="space-y-4">
    <p className="text-gray-700">{message}</p>
    <div className="flex gap-2">
      <Button variant={variant} onClick={onConfirm} className="flex-1">
        {confirmText}
      </Button>
      <Button variant="outline" onClick={onCancel} className="flex-1">
        {cancelText}
      </Button>
    </div>
  </div>
);
