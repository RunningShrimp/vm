// 宏定义（Macros）
//
// 本模块提供VM开发常用的宏定义：
// - 调试宏
// - 断言宏
// - 度量宏
// - 日志宏

/// 调试宏：在debug模式打印信息
#[macro_export]
macro_rules! debug_msg {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            println!("[DEBUG] {}", format!($($arg)*));
        }
    }
}

/// 调试宏：在debug模式打印详细信息
#[macro_export]
macro_rules! debug_verbose {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            eprintln!("[DEBUG-VERBOSE] {}", format!($($arg)*));
        }
    }
}

/// 信息宏：打印信息
#[macro_export]
macro_rules! info_msg {
    ($($arg:tt)*) => {
        println!("[INFO] {}", format!($($arg)*));
    }
}

/// 警告宏：打印警告
#[macro_export]
macro_rules! warn_msg {
    ($($arg:tt)*) => {
        eprintln!("[WARN] {}", format!($($arg)*));
    }
}

/// 错误宏：打印错误
#[macro_export]
macro_rules! error_msg {
    ($($arg:tt)*) => {
        eprintln!("[ERROR] {}", format!($($arg)*));
    }
}

/// 度量宏：度量代码块执行时间
#[macro_export]
macro_rules! measure_time {
    ($label:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        info_msg!("[PERF] {} took {:?}", $label, duration);
        result
    }};
}

/// 度量宏：度量函数执行时间（返回值）
#[macro_export]
macro_rules! measure_fn {
    ($label:expr, $fn:ident) => {{
        let start = std::time::Instant::now();
        let result = $fn();
        let duration = start.elapsed();
        info_msg!("[PERF] {}() took {:?}", $label, duration);
        result
    }};
}

/// 度量宏：度量代码块执行时间（返回值和时长）
#[macro_export]
macro_rules! measure_time_with_result {
    ($label:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        (result, duration)
    }};
}

/// 断言宏：在debug模式断言
#[macro_export]
macro_rules! assert_debug {
    ($cond:expr) => {
        #[cfg(debug_assertions)]
        {
            if !($cond) {
                panic!("Assertion failed: {}", stringify!($cond));
            }
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        #[cfg(debug_assertions)]
        {
            if !($cond) {
                panic!("Assertion failed: {} ({})", stringify!($cond), format!($($arg)+));
            }
        }
    }
}

/// 断言宏：在debug模式断言并返回值
#[macro_export]
macro_rules! assert_debug_ret {
    ($cond:expr, $val:expr) => {
        #[cfg(debug_assertions)]
        {
            if !($cond) {
                panic!("Assertion failed: {}", stringify!($cond));
            }
            $val
        }
        #[cfg(not(debug_assertions))]
        {
            $val
        }
    };
}

/// 不可达宏：标记不可达的代码路径
#[macro_export]
macro_rules! unreachable_code {
    ($msg:expr) => {
        unreachable!("{}", $msg)
    };
    () => {
        unreachable!()
    };
}

/// 未实现宏：标记未实现的功能
#[macro_export]
macro_rules! not_implemented {
    () => {
        panic!("Not implemented yet")
    };
    ($msg:expr) => {
        panic!("Not implemented: {}", $msg)
    };
}

/// TODO宏：标记待实现的功能
#[macro_export]
macro_rules! todo {
    () => {
        eprintln!("[TODO] This feature is not yet implemented")
    };
    ($msg:expr) => {
        eprintln!("[TODO] {}", $msg)
    };
}

/// FIXME宏：标记需要修复的代码
#[macro_export]
macro_rules! fixme {
    () => {
        eprintln!("[FIXME] This code needs to be fixed")
    };
    ($msg:expr) => {
        eprintln!("[FIXME] {}", $msg)
    };
}

/// XXX宏：标记需要改进的代码
#[macro_export]
macro_rules! xxx {
    () => {
        eprintln!("[XXX] This code needs improvement")
    };
    ($msg:expr) => {
        eprintln!("[XXX] {}", $msg)
    };
}

/// HACK宏：标记临时解决方案
#[macro_export]
macro_rules! hack {
    () => {
        eprintln!("[HACK] This is a temporary solution")
    };
    ($msg:expr) => {
        eprintln!("[HACK] {}", $msg)
    };
}

/// 重复宏：重复执行代码块N次
#[macro_export]
macro_rules! repeat_n {
    ($n:expr, $block:expr) => {
        for _ in 0..$n {
            $block
        }
    };
}

/// for_each宏：遍历数组并执行代码块
#[macro_export]
macro_rules! for_each {
    ($array:expr, $item:ident, $block:expr) => {
        for $item in $array {
            $block
        }
    };
}

/// 可选宏：安全地获取Option值或返回默认值
#[macro_export]
macro_rules! unwrap_or {
    ($opt:expr, $default:expr) => {
        match $opt {
            Some(val) => val,
            None => $default,
        }
    };
}

/// 可选宏：安全地获取Option值或panic
#[macro_export]
macro_rules! unwrap_expect {
    ($opt:expr, $msg:expr) => {
        match $opt {
            Some(val) => val,
            None => panic!("{}", $msg),
        }
    };
}

/// 条件编译宏：根据特性选择不同的实现
#[macro_export]
macro_rules! cfg_feature {
    ($feature:expr, $if_true:expr, $if_false:expr) => {
        #[cfg(feature = $feature)]
        $if_true

        #[cfg(not(feature = $feature))]
        $if_false
    }
}

/// 延迟求值宏：延迟计算表达式
#[macro_export]
macro_rules! lazy {
    ($expr:expr) => {
        || $expr
    };
}

/// 常量宏：标记恒定的表达式
#[macro_export]
macro_rules! const_fn {
    () => {
        const
    }
}

/// 静态宏：标记静态的变量
#[macro_export]
macro_rules! static_var {
    () => {
        static
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_measure_time() {
        let result = measure_time!("test", {
            std::thread::sleep(std::time::Duration::from_millis(10));
            42
        });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_measure_fn() {
        fn test_fn() -> i32 {
            42
        }
        let result = measure_fn!("test_fn", test_fn);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_measure_time_with_result() {
        let (result, duration) = measure_time_with_result!("test", {
            std::thread::sleep(std::time::Duration::from_millis(10));
            42
        });
        assert_eq!(result, 42);
        assert!(duration.as_millis() >= 10);
    }

    #[test]
    fn test_repeat_n() {
        let mut counter = 0;
        repeat_n!(3, {
            counter += 1;
        });
        assert_eq!(counter, 3);
    }

    #[test]
    fn test_for_each() {
        let array = vec![1, 2, 3, 4, 5];
        let mut sum = 0;
        for_each!(array, item, {
            sum += item;
        });
        assert_eq!(sum, 15);
    }

    #[test]
    fn test_unwrap_or() {
        let some_val: Option<i32> = Some(42);
        let none_val: Option<i32> = None;

        assert_eq!(unwrap_or!(some_val, 0), 42);
        assert_eq!(unwrap_or!(none_val, 0), 0);
    }
}
