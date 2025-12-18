/**
 * 原子化组件库 - Atomic Design
 * 模仿 VMware Workstation 界面风格
 */

import React from 'react';

// ============ 颜色定义 ============
export const colors = {
  primary: '#3B5998',      // VMware 蓝
  secondary: '#5A7A99',
  success: '#28a745',
  warning: '#ffc107',
  danger: '#dc3545',
  info: '#17a2b8',
  light: '#f8f9fa',
  dark: '#212529',
  muted: '#6c757d',
  border: '#dee2e6',
  bg: '#ffffff',
  bgAlt: '#f5f7fa',
};

// ============ 原子组件 ============

/** 按钮 - 各种样式 */
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'success' | 'danger' | 'outline' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
  disabled?: boolean;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = 'primary', size = 'md', className, ...props }, ref) => {
    const baseStyle = 'font-medium rounded transition-colors duration-200 cursor-pointer border';

    const variantStyles = {
      primary: 'bg-blue-600 text-white border-blue-600 hover:bg-blue-700',
      secondary: 'bg-gray-600 text-white border-gray-600 hover:bg-gray-700',
      success: 'bg-green-600 text-white border-green-600 hover:bg-green-700',
      danger: 'bg-red-600 text-white border-red-600 hover:bg-red-700',
      outline: 'bg-transparent text-gray-700 border-gray-400 hover:bg-gray-100',
      ghost: 'bg-transparent text-gray-700 border-transparent hover:bg-gray-100',
    };

    const sizeStyles = {
      sm: 'px-2 py-1 text-xs',
      md: 'px-4 py-2 text-sm',
      lg: 'px-6 py-3 text-base',
    };

    return (
      <button
        ref={ref}
        className={`${baseStyle} ${variantStyles[variant]} ${sizeStyles[size]} ${className || ''}`}
        {...props}
      />
    );
  }
);

Button.displayName = 'Button';

/** 输入框 */
export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ label, error, className, ...props }, ref) => (
    <div className="w-full">
      {label && <label className="block text-sm font-medium text-gray-700 mb-1">{label}</label>}
      <input
        ref={ref}
        className={`w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:ring-2 focus:ring-blue-500 focus:border-transparent outline-none transition ${
          error ? 'border-red-500' : ''
        } ${className || ''}`}
        {...props}
      />
      {error && <p className="text-red-500 text-sm mt-1">{error}</p>}
    </div>
  )
);

Input.displayName = 'Input';

/** 卡片 */
export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  title?: string;
  subtitle?: string;
}

export const Card = React.forwardRef<HTMLDivElement, CardProps>(
  ({ title, subtitle, children, className, ...props }, ref) => (
    <div
      ref={ref}
      className={`bg-white rounded-lg border border-gray-200 shadow-sm hover:shadow-md transition ${className || ''}`}
      {...props}
    >
      {(title || subtitle) && (
        <div className="px-6 py-4 border-b border-gray-200">
          {title && <h3 className="text-lg font-semibold text-gray-900">{title}</h3>}
          {subtitle && <p className="text-sm text-gray-500 mt-1">{subtitle}</p>}
        </div>
      )}
      <div className="px-6 py-4">{children}</div>
    </div>
  )
);

Card.displayName = 'Card';

/** 徽章 */
export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: 'primary' | 'success' | 'warning' | 'danger' | 'info';
}

export const Badge = React.forwardRef<HTMLSpanElement, BadgeProps>(
  ({ variant = 'primary', children, className, ...props }, ref) => {
    const variantStyles = {
      primary: 'bg-blue-100 text-blue-800',
      success: 'bg-green-100 text-green-800',
      warning: 'bg-yellow-100 text-yellow-800',
      danger: 'bg-red-100 text-red-800',
      info: 'bg-cyan-100 text-cyan-800',
    };

    return (
      <span
        ref={ref}
        className={`inline-block px-3 py-1 rounded-full text-xs font-semibold ${variantStyles[variant]} ${className || ''}`}
        {...props}
      >
        {children}
      </span>
    );
  }
);

Badge.displayName = 'Badge';

/** 标签页 */
export interface TabsProps {
  tabs: Array<{ label: string; value: string; icon?: React.ReactNode }>;
  activeTab: string;
  onChange: (tab: string) => void;
}

export const Tabs: React.FC<TabsProps> = ({ tabs, activeTab, onChange }) => (
  <div className="flex border-b border-gray-200 bg-gray-50">
    {tabs.map((tab) => (
      <button
        key={tab.value}
        onClick={() => onChange(tab.value)}
        className={`px-4 py-3 font-medium text-sm transition-colors ${
          activeTab === tab.value
            ? 'text-blue-600 border-b-2 border-blue-600 bg-white'
            : 'text-gray-600 hover:text-gray-900'
        }`}
      >
        {tab.icon && <span className="mr-2">{tab.icon}</span>}
        {tab.label}
      </button>
    ))}
  </div>
);

/** 进度条 */
export interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value: number;
  max?: number;
  variant?: 'primary' | 'success' | 'warning' | 'danger';
}

export const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  ({ value, max = 100, variant = 'primary', className, ...props }, ref) => {
    const percentage = (value / max) * 100;
    const variantColors = {
      primary: 'bg-blue-600',
      success: 'bg-green-600',
      warning: 'bg-yellow-600',
      danger: 'bg-red-600',
    };

    return (
      <div ref={ref} className={`w-full bg-gray-200 rounded-full h-2 overflow-hidden ${className || ''}`} {...props}>
        <div
          className={`h-full ${variantColors[variant]} transition-all duration-300`}
          style={{ width: `${Math.min(percentage, 100)}%` }}
        />
      </div>
    );
  }
);

Progress.displayName = 'Progress';

/** 加载旋转器 */
export interface SpinnerProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: 'sm' | 'md' | 'lg';
}

export const Spinner = React.forwardRef<HTMLDivElement, SpinnerProps>(
  ({ size = 'md', className, ...props }, ref) => {
    const sizeStyles = {
      sm: 'w-4 h-4',
      md: 'w-8 h-8',
      lg: 'w-12 h-12',
    };

    return (
      <div
        ref={ref}
        className={`${sizeStyles[size]} border-4 border-gray-300 border-t-blue-600 rounded-full animate-spin ${className || ''}`}
        {...props}
      />
    );
  }
);

Spinner.displayName = 'Spinner';

/** 工具提示 */
export interface TooltipProps {
  content: React.ReactNode;
  children: React.ReactNode;
  position?: 'top' | 'bottom' | 'left' | 'right';
}

export const Tooltip: React.FC<TooltipProps> = ({ content, children, position = 'top' }) => {
  const positionStyles = {
    top: 'bottom-full mb-2',
    bottom: 'top-full mt-2',
    left: 'right-full mr-2',
    right: 'left-full ml-2',
  };

  return (
    <div className="relative group cursor-help inline-block">
      {children}
      <div
        className={`absolute ${positionStyles[position]} hidden group-hover:block bg-gray-900 text-white text-xs px-2 py-1 rounded whitespace-nowrap z-10`}
      >
        {content}
      </div>
    </div>
  );
};

/** 模态框 */
export interface ModalProps {
  isOpen: boolean;
  title?: string;
  onClose: () => void;
  children: React.ReactNode;
  size?: 'sm' | 'md' | 'lg';
}

export const Modal: React.FC<ModalProps> = ({ isOpen, title, onClose, children, size = 'md' }) => {
  if (!isOpen) return null;

  const sizeStyles = {
    sm: 'max-w-sm',
    md: 'max-w-md',
    lg: 'max-w-lg',
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className={`bg-white rounded-lg shadow-lg ${sizeStyles[size]}`}>
        {title && (
          <div className="px-6 py-4 border-b border-gray-200 flex justify-between items-center">
            <h2 className="text-lg font-semibold">{title}</h2>
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-gray-600 transition"
            >
              ✕
            </button>
          </div>
        )}
        <div className="px-6 py-4">{children}</div>
      </div>
    </div>
  );
};

/** 下拉菜单 */
export interface DropdownProps {
  trigger: React.ReactNode;
  items: Array<{ label: string; value: string; icon?: React.ReactNode; onClick?: () => void }>;
}

export const Dropdown: React.FC<DropdownProps> = ({ trigger, items }) => {
  const [isOpen, setIsOpen] = React.useState(false);
  const menuRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <div ref={menuRef} className="relative inline-block">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="inline-flex items-center"
      >
        {trigger}
      </button>
      {isOpen && (
        <div className="absolute right-0 mt-2 w-48 bg-white rounded-lg shadow-lg border border-gray-200 z-10">
          {items.map((item) => (
            <button
              key={item.value}
              onClick={() => {
                item.onClick?.();
                setIsOpen(false);
              }}
              className="w-full text-left px-4 py-2 hover:bg-gray-100 transition flex items-center"
            >
              {item.icon && <span className="mr-2">{item.icon}</span>}
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};
