# Lab 3：进程管理

## 实现概述

迁移用户缓冲区时间写入和内存映射；spawn 直接从 ELF 创建子进程，设置父子关系和初始上下文，不复制父进程地址空间。就绪队列改用 stride 调度，优先级默认为 16，拒绝小于 2 的值，按 BigStride/priority 增加已选任务的 stride。

## 设计与实现

spawn 通过程序名找到 ELF，调用新进程构造器，登记父进程的 children 和子进程的 Weak parent，再加入就绪队列。新进程拥有独立地址空间、PID 和内核栈；exec 同时重置堆底和 program break。

调度器扫描就绪队列选最小 stride，先记账再运行；同 stride 时保持队列中的先后顺序。实际 stride 使用 u128，BigStride=2^64，避免课程运行时出现 u64 回绕，并让最大合法 isize 优先级仍具有非零步长。

## 问答：stride 回绕

对于 8 位计数，p2 从 250 加 10 会回绕到 4。直接比较整数会错误地继续选 p2，而逻辑上应选 stride=255 的 p1。必须采用跨回绕的相对顺序或更宽的计数。

在所有任务始终参与同一调度集合、从相同初值开始且每次选择最小者时，每次增长不超过 BigStride/2，因此当前最大值与最小值之差不会超过这一范围。新加入、长期休眠和重新唤醒的任务需要额外定义基准，不能不加条件套用此结论。

若使用 u64 和 BinaryHeap 维护“最小逻辑 stride 在堆顶”，在题目限定的有效距离范围内，可比较 self.0.wrapping_sub(other.0)：非零差小于 2^63 时，self 逻辑上更大，返回 Ordering::Less；否则返回 Ordering::Greater。BinaryHeap 是最大堆，故要反转次序。相等时应返回 Equal，以维持正常容器约定。此处为问答分析；实际实现使用 u128 线性扫描，不声称实现了该 BinaryHeap。

## 验证记录

2026-10-26：使用课程 Docker 环境中的未修改官方评分脚本执行第三项实验。结果为 **15/15，passed=true，检查器退出码 0**，本章要求的 lab1～lab3 报告检查通过。

检查器提交：`7d61ec55b58eed6ca7052917846c1b87af34563a`；测试程序提交：`a0593662ad55d670ba8c27ce1763347cd0dd552f`。完整日志：`tmp/codex-validation/runs/ch5-first-1/tmp/rcore-grade.log`。线上成绩需另行核对 Actions 与 OpenCamp。

参考：https://learningos.cn/rCore-Tutorial-Guide/chapter5/4exercise.html

## 协作、参考资料与荣誉准则说明

1. 本次新增代码、调试分析和报告由 OpenAI Codex 在学员授权下协助完成；学员负责启动本地 WSL/Docker 环境。这里明确披露 AI 协助范围，不把这些工作声明为学员独立完成。
2. 使用课程提供的章节模板、官方实验指导和官方测试程序作为接口及行为依据；未采用其他学员的实验答案。新增实现所在模块注明了 AI 协助与参考章节。
3. 实验记录只报告实际执行的命令和结果，不编造个人学习用时、测试成功记录或独立完成声明。AI 协助是否符合课程要求及其对评分的影响，以课程方规定为准。
4. 本仓库代码用于本人领取的课程实验和评测，无意破坏或妨碍计算机系统；不向其他学员传播实验答案。上游模板保留原许可证与版权信息。

荣誉准则原文：https://learningos.cn/rCore-Tutorial-Guide/honorcode.html
