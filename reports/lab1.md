# Lab 1：多道程序与分时多任务

## 实现概述

在统一系统调用入口先统计调用次数，再分派具体功能，使查询包含本次 trace 调用。计数存放在各任务的控制块中，任务切换不会混淆计数。sys_trace 支持按字节读写当前任务地址、查询调用次数，非法请求返回 -1；本章依照题目要求暂不提供地址空间隔离。

## 设计与实现

- 修改 syscall/mod.rs、syscall/process.rs、task/mod.rs 和 task/task.rs。
- 以系统调用 ID 到计数槽位的映射保存内核支持的调用；尚未调用或不支持的 ID 查询返回 0。统计在退出、让出 CPU 等改变控制流的操作之前进行。
- 字节访问使用 volatile 读写，写入只保留 data 的低 8 位。访问权限检查在下一章引入。

## 问答

### U 态执行特权操作会怎样

U 态执行只能在更高特权级使用的指令，或读取 sstatus 等 S 态 CSR，会产生 IllegalInstruction，陷入内核并终止相应应用。越权访存则取决于访问类型和是否启用地址翻译，可能产生访问异常或页异常。课程固件为 RustSBI 0.4.1；具体测试输出见本章验证记录，不将预期行为冒充额外执行的测试。

### __alltraps 与 __restore

1. 刚进入 __restore 时，sp 指向内核栈上的 TrapContext。两种情景是任务首次启动时恢复预先构造的上下文，以及应用陷入内核并完成处理后恢复原上下文。
2. 偏移 32×8、33×8、2×8 对应 sstatus、sepc 和用户 sp。sstatus 的 SPP 决定 sret 返回的特权级，SPIE 影响返回后的中断使能；sepc 决定用户继续执行的位置；sscratch 暂存用户 sp，以便最后交换栈指针。
3. x2 是 sp，需要在其他寄存器加载和栈帧释放完成后单独恢复；本模板约定应用不使用 x4/tp，因此未在普通循环中保存恢复它。这不是通用 ABI 下可以永久忽略 tp 的结论。
4. __restore 最后执行 csrrw sp, sscratch, sp 后，sp 为用户栈指针，sscratch 为释放陷阱帧后的内核栈顶。
5. 真正改变特权级的是 sret；在保存的 SPP=U 时，它将 PC 设为 sepc 并返回 U 态。
6. __alltraps 开头相同的交换指令执行后，sp 为内核栈顶，sscratch 为刚离开的用户栈指针。
7. 系统调用由 ecall 触发 U→S 陷阱；异常和被委托的中断也能触发该切换，CPU 按 stvec 进入陷阱入口，而不是由普通函数调用切换。

## 验证记录

2026-10-26：在 QEMU 8.2.2、nightly-2024-05-02、cargo-binutils 0.4.0 的课程 Docker 环境中执行未修改的 `.github/scripts/rcore_grade.py`，使用课程固定版本的官方检查器和测试程序。结果为 **7/7，passed=true，检查器退出码 0**；本章全部必需报告的存在检查通过。

检查器提交：`7d61ec55b58eed6ca7052917846c1b87af34563a`；测试程序提交：`a0593662ad55d670ba8c27ce1763347cd0dd552f`。完整日志保存在作业仓库的 `tmp/codex-validation/runs/ch3-second-1/tmp/rcore-grade.log`。这属于本地真实评测结果，GitHub Actions 与 OpenCamp 同步状态另行核对。

参考：https://learningos.cn/rCore-Tutorial-Guide/chapter3/5exercise.html

## 协作、参考资料与荣誉准则说明

1. 本次新增代码、调试分析和报告由 OpenAI Codex 在学员授权下协助完成；学员负责启动本地 WSL/Docker 环境。这里明确披露 AI 协助范围，不把这些工作声明为学员独立完成。
2. 使用课程提供的章节模板、官方实验指导和官方测试程序作为接口及行为依据；未采用其他学员的实验答案。新增实现所在模块注明了 AI 协助与参考章节。
3. 实验记录只报告实际执行的命令和结果，不编造个人学习用时、测试成功记录或独立完成声明。AI 协助是否符合课程要求及其对评分的影响，以课程方规定为准。
4. 本仓库代码用于本人领取的课程实验和评测，无意破坏或妨碍计算机系统；不向其他学员传播实验答案。上游模板保留原许可证与版权信息。

荣誉准则原文：https://learningos.cn/rCore-Tutorial-Guide/honorcode.html
