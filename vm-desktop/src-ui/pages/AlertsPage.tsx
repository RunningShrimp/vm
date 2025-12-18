import React, { useState } from 'react';
import { Button, Badge, Input } from '../atoms';
import { CardLayout, GridLayout } from '../templates';

interface Alert {
  id: string;
  title: string;
  severity: 'critical' | 'warning' | 'info';
  vm: string;
  timestamp: string;
  message: string;
  status: 'active' | 'resolved';
}

interface AlertRule {
  id: string;
  name: string;
  condition: string;
  threshold: number;
  duration: number;
  enabled: boolean;
}

export const AlertsPage: React.FC = () => {
  const [alerts, setAlerts] = useState<Alert[]>([
    {
      id: 'alert-1',
      title: 'CPU 使用率过高',
      severity: 'warning',
      vm: 'web-server',
      timestamp: '2025-12-11 10:30:45',
      message: 'CPU 使用率持续超过 90% 超过 5 分钟',
      status: 'active',
    },
    {
      id: 'alert-2',
      title: '磁盘空间不足',
      severity: 'critical',
      vm: 'db-server',
      timestamp: '2025-12-11 09:15:20',
      message: '磁盘使用率达到 95%，即将填满',
      status: 'active',
    },
    {
      id: 'alert-3',
      title: '内存泄漏预警',
      severity: 'warning',
      vm: 'app-server',
      timestamp: '2025-12-11 08:00:00',
      message: '内存占用持续增长，可能存在泄漏',
      status: 'resolved',
    },
    {
      id: 'alert-4',
      title: '网络连接异常',
      severity: 'info',
      vm: 'monitor-vm',
      timestamp: '2025-12-11 07:45:30',
      message: '网络丢包率: 2.5%',
      status: 'resolved',
    },
  ]);

  const [rules, setRules] = useState<AlertRule[]>([
    {
      id: 'rule-1',
      name: 'CPU 使用率告警',
      condition: 'CPU > 80%',
      threshold: 80,
      duration: 5,
      enabled: true,
    },
    {
      id: 'rule-2',
      name: '内存使用率告警',
      condition: 'Memory > 90%',
      threshold: 90,
      duration: 3,
      enabled: true,
    },
    {
      id: 'rule-3',
      name: '磁盘使用率告警',
      condition: 'Disk > 85%',
      threshold: 85,
      duration: 10,
      enabled: true,
    },
  ]);

  const [showAlertConfig, setShowAlertConfig] = useState(false);
  const [filterSeverity, setFilterSeverity] = useState<'all' | 'critical' | 'warning' | 'info'>('all');
  const [filterStatus, setFilterStatus] = useState<'all' | 'active' | 'resolved'>('all');

  const filteredAlerts = alerts.filter(alert => {
    const matchesSeverity = filterSeverity === 'all' || alert.severity === filterSeverity;
    const matchesStatus = filterStatus === 'all' || alert.status === filterStatus;
    return matchesSeverity && matchesStatus;
  });

  const activeAlerts = alerts.filter(a => a.status === 'active');
  const criticalCount = activeAlerts.filter(a => a.severity === 'critical').length;
  const warningCount = activeAlerts.filter(a => a.severity === 'warning').length;

  const getSeverityColor = (severity: string) => {
    switch (severity) {
      case 'critical':
        return 'text-red-600';
      case 'warning':
        return 'text-amber-600';
      default:
        return 'text-blue-600';
    }
  };

  const getSeverityBgColor = (severity: string) => {
    switch (severity) {
      case 'critical':
        return 'bg-red-50';
      case 'warning':
        return 'bg-amber-50';
      default:
        return 'bg-blue-50';
    }
  };

  return (
    <div className="space-y-6">
      {/* 统计卡片 */}
      <GridLayout columns={4} gap="md">
        <CardLayout>
          <div className="text-center">
            <div className="text-3xl font-bold text-red-600">{criticalCount}</div>
            <div className="text-sm text-gray-600">严重告警</div>
          </div>
        </CardLayout>
        <CardLayout>
          <div className="text-center">
            <div className="text-3xl font-bold text-amber-600">{warningCount}</div>
            <div className="text-sm text-gray-600">警告告警</div>
          </div>
        </CardLayout>
        <CardLayout>
          <div className="text-center">
            <div className="text-3xl font-bold text-green-600">{alerts.filter(a => a.status === 'resolved').length}</div>
            <div className="text-sm text-gray-600">已解决</div>
          </div>
        </CardLayout>
        <CardLayout>
          <div className="text-center">
            <div className="text-3xl font-bold text-blue-600">{alerts.length}</div>
            <div className="text-sm text-gray-600">告警总数</div>
          </div>
        </CardLayout>
      </GridLayout>

      {/* 过滤栏 */}
      <div className="flex gap-4">
        <div className="flex gap-2">
          <Button
            variant={filterSeverity === 'all' ? 'primary' : 'outline'}
            onClick={() => setFilterSeverity('all')}
          >
            全部
          </Button>
          <Button
            variant={filterSeverity === 'critical' ? 'primary' : 'outline'}
            onClick={() => setFilterSeverity('critical')}
          >
            🔴 严重
          </Button>
          <Button
            variant={filterSeverity === 'warning' ? 'primary' : 'outline'}
            onClick={() => setFilterSeverity('warning')}
          >
            🟠 警告
          </Button>
          <Button
            variant={filterSeverity === 'info' ? 'primary' : 'outline'}
            onClick={() => setFilterSeverity('info')}
          >
            🔵 信息
          </Button>
        </div>
        <div className="flex gap-2 ml-auto">
          <Button
            variant={filterStatus === 'all' ? 'primary' : 'outline'}
            onClick={() => setFilterStatus('all')}
          >
            全部状态
          </Button>
          <Button
            variant={filterStatus === 'active' ? 'primary' : 'outline'}
            onClick={() => setFilterStatus('active')}
          >
            活跃
          </Button>
          <Button
            variant={filterStatus === 'resolved' ? 'primary' : 'outline'}
            onClick={() => setFilterStatus('resolved')}
          >
            已解决
          </Button>
        </div>
      </div>

      {/* 告警列表 */}
      <div className="space-y-2">
        {filteredAlerts.map(alert => (
          <div
            key={alert.id}
            className={`p-4 rounded-lg border border-gray-200 ${getSeverityBgColor(alert.severity)}`}
          >
            <div className="flex items-start justify-between gap-4">
              <div className="flex-1">
                <div className="flex items-center gap-3 mb-2">
                  <div
                    className={`text-2xl ${getSeverityColor(alert.severity)}`}
                  >
                    {alert.severity === 'critical' && '🔴'}
                    {alert.severity === 'warning' && '🟠'}
                    {alert.severity === 'info' && '🔵'}
                  </div>
                  <div>
                    <h4 className="font-semibold text-gray-900">{alert.title}</h4>
                    <p className="text-sm text-gray-600">{alert.vm}</p>
                  </div>
                </div>
                <p className="text-sm text-gray-700 ml-11 mb-2">{alert.message}</p>
                <div className="flex items-center gap-4 ml-11 text-xs text-gray-500">
                  <span>🕐 {alert.timestamp}</span>
                  <Badge variant={alert.status === 'active' ? 'warning' : 'default'}>
                    {alert.status === 'active' ? '活跃' : '已解决'}
                  </Badge>
                </div>
              </div>
              <div className="flex gap-2">
                {alert.status === 'active' && (
                  <Button size="sm" variant="outline">
                    ✓ 标记已解决
                  </Button>
                )}
                <Button size="sm" variant="ghost">
                  🔍 详情
                </Button>
              </div>
            </div>
          </div>
        ))}
      </div>

      {filteredAlerts.length === 0 && (
        <div className="text-center py-12 bg-white rounded-lg border border-gray-200">
          <span className="text-5xl mb-4 block">😊</span>
          <h3 className="text-lg font-semibold text-gray-900 mb-2">没有告警</h3>
          <p className="text-gray-600">系统运行正常</p>
        </div>
      )}

      {/* 告警规则配置 */}
      <CardLayout
        title="告警规则"
        actions={
          <Button size="sm" onClick={() => setShowAlertConfig(!showAlertConfig)}>
            {showAlertConfig ? '关闭' : '设置'}
          </Button>
        }
      >
        {!showAlertConfig ? (
          <div className="space-y-2">
            {rules.map(rule => (
              <div
                key={rule.id}
                className="flex items-center justify-between p-3 bg-gray-50 rounded-lg"
              >
                <div className="flex-1">
                  <div className="font-medium text-gray-900">{rule.name}</div>
                  <div className="text-sm text-gray-600">
                    条件: {rule.condition} | 持续时间: {rule.duration} 分钟
                  </div>
                </div>
                <div className="flex gap-2">
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={rule.enabled}
                      onChange={() => {
                        setRules(prev =>
                          prev.map(r =>
                            r.id === rule.id ? { ...r, enabled: !r.enabled } : r
                          )
                        );
                      }}
                      className="w-4 h-4"
                    />
                    <span className="text-sm">{rule.enabled ? '启用' : '禁用'}</span>
                  </label>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="space-y-4">
            <h4 className="font-semibold text-gray-900">添加新规则</h4>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">规则名称</label>
              <Input placeholder="例如: 网络延迟告警" />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">告警条件</label>
              <select className="w-full px-3 py-2 border border-gray-300 rounded-md">
                <option>CPU > %</option>
                <option>Memory > %</option>
                <option>Disk > %</option>
                <option>Network Latency > ms</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">阈值</label>
              <input type="number" placeholder="80" className="w-full px-3 py-2 border border-gray-300 rounded-md" />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">持续时间 (分钟)</label>
              <input type="number" placeholder="5" className="w-full px-3 py-2 border border-gray-300 rounded-md" />
            </div>
            <div className="flex gap-2">
              <Button variant="primary" className="flex-1">
                创建规则
              </Button>
              <Button variant="outline" onClick={() => setShowAlertConfig(false)}>
                取消
              </Button>
            </div>
          </div>
        )}
      </CardLayout>
    </div>
  );
};
