#!/usr/bin/env python3
"""修复vm-frontend-x86_64/src/lib.rs中的if_same_then_else警告"""

def fix_file():
    with open('vm-frontend-x86_64/src/lib.rs', 'r') as f:
        lines = f.readlines()

    modified_lines = lines.copy()
    offset = 0

    # 问题1：第3090行附近 - 两个条件分支都返回None
    for i in range(len(lines)):
        if 'let final_base = if mod_ == 0 && rm == 5 && !has_sib {' in lines[i]:
            print(f"Found first problem at line {i+1}")
            # 跳过注释行
            j = i
            while j + 12 < len(lines):
                # 查找完整的if-else if-else块
                block = ''.join(lines[j:j+12])
                if 'let final_base = if mod_ == 0 && rm == 5 && !has_sib' in lines[j]:
                    if lines[j+1].strip() == 'None' and lines[j+2].strip() == '}':
                        if 'else if mod_ == 0 && has_sib' in lines[j+3]:
                            if lines[j+4].strip() == 'None' and lines[j+5].strip() == '}':
                                if 'else {' in lines[j+6]:
                                    if lines[j+7].strip() == 'base' and lines[j+8].strip() == '};':
                                        print(f"Found complete first problem block at line {j+1}")
                                        # 替换为合并后的版本
                                        replacement = [
                                            '                    let final_base = if mod_ == 0 && (rm == 5 && !has_sib || has_sib && (base.expect("Operation failed") & 7) == 5) {',
                                            '                        None',
                                            '                    } else {',
                                            '                        base',
                                            '                    };',
                                            '\n'  # 保留原有的空行
                                        ]
                                        # 检查原块后面的内容
                                        start = j + offset
                                        end = j + offset + 9  # 包括};和后面的空行
                                        # 替换
                                        modified_lines = modified_lines[:start] + replacement + modified_lines[end:]
                                        offset += len(replacement) - 9
                                        break
                j += 1
            break

    # 问题2：第3171行附近 - 类似的模式
    lines = modified_lines.copy()  # 使用修改后的内容
    for i in range(len(lines)):
        if 'let final_base = if mod_ == 0 && rm == 5 && !has_sib' in lines[i]:
            print(f"Found second problem at line {i+1}")
            j = i
            while j + 12 < len(lines):
                block = ''.join(lines[j:j+12])
                if 'let final_base = if mod_ == 0 && rm == 5 && !has_sib' in lines[j]:
                    if lines[j+1].strip() == 'None' and lines[j+2].strip() == '}':
                        if 'else if mod_ == 0 && has_sib' in lines[j+3]:
                            if lines[j+4].strip() == 'None' and lines[j+5].strip() == '}':
                                if 'else {' in lines[j+6]:
                                    if lines[j+7].strip() == 'base' and lines[j+8].strip() == '};':
                                        print(f"Found complete second problem block at line {j+1}")
                                        replacement = [
                                            '                    let final_base = if mod_ == 0 && (rm == 5 && !has_sib || has_sib && (base.expect("Operation failed") & 7) == 5) {',
                                            '                        None',
                                            '                    } else {',
                                            '                        base',
                                            '                    };',
                                            '\n'
                                        ]
                                        start = j + offset
                                        end = j + offset + 9
                                        modified_lines = modified_lines[:start] + replacement + modified_lines[end:]
                                        offset += len(replacement) - 9
                                        break
                j += 1
            break

    # 写回文件
    with open('vm-frontend-x86_64/src/lib.rs', 'w') as f:
        f.writelines(modified_lines)

    print("Fixed vm-frontend-x86_64/src/lib.rs")

if __name__ == '__main__':
    fix_file()
