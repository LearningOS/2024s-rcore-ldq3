# 功能实现
函数 sys_task_info 的签名表明任务是向传入的裸指针指向的 TaskInfo 结构写入系统当前执行任务的信息

考虑该系统调用是否一定能被正确执行：当前任务的信息是一定存在的，只有传入的裸指针可能导致该系统调用执行失败

获取任务状态相对简单

获取任务使用的系统调用及调用次数：需要在内核中创建相应的结构用于保存这些信息。考虑到内核需要为所有的任务维护这样一个结构，但事实上绝大多数任务只会用到一小部分系统调用，在这样的情况下使用一个数组过于浪费空间，应该使用 map 结构。另外，考虑到 TaskInfo 中用一个桶数组来存储这些信息，为了能简单实现 map 到数组的转换，可以使用 BTreeMap 数据结构。

获取任务的开始时间：增加一个 start_time 字段

# 简答作业
## 1
**环境说明**：QEMU 7.0.0、随实验仓库克隆的原始 rustsbi-qemu.bin

ch2b_bad_address.rs: 尝试向地址 0 处的内存写入一个字节的值 0，这将触发存数访问异常

ch2b_bad_instructions.rs：尝试执行 sret 指令，由于这是一个 S 态指令，运行在 U 态的用户程序执行该指令将触发非法指令异常

ch2b_bad_register.rs: 尝试访问寄存器 sstatus，这将触发非法指令异常

## 2
__alltraps：在异常或中断发生时，保存当前的上下文信息到内核栈中，并调用一个处理函数来处理异常或中断情况

__restore：在异常或中断处理完成后，恢复先前保存的上下文信息

1. 答：在 risc-v 的寄存器调用约定中，a0 用于保存函数的传入参数或返回值。在 ch3 中，__restore 函数不在需要使用 a0 来传入参数。在 ch2 中，__restore 函数将 a0 中值设置为栈指针。由于 __restore 主要用于以下两个场景，所以 a0 中的值为具体模式的栈指针：
- 由高级模式返回低级模式，如系统调用完成之后由 S mode 回到 U mode
- 由低级模式进入高级模式，如请求系统调用时由 U mode 进入 S mode

2. 答：sstatus（Supervisor Status）用于保存 S mode 的状态信息，spec（Supervisor Exception PC）指向发生异常的指令，sscratch（Supervisor Scratch）向异常处理程序提供一个字的临时存储

3. 答：x2 的 ABI name 即 sp，x4 的 ABI name 为 tp，意为 thread pointer

4. 答：该指令用于交换 sscratch 和 sp 的值，交换后 sp 指向 user stack 的栈顶，sscratch 指向 kernel stack 的栈顶

5. 答：指令 sret 的作用从监管模式的异常处理程序返回。将 pc 设 为 CSRs\[sepc]， 将 特 权 模 式 设 为 CSRs\[sstatus].SPP， 将 CSRs\[sstatus].SIE 设 为 CSRs\[sstatus].SPIE， 将 CSRs\[sstatus].SPIE 设为 1，将 CSRs\[sstatus].SPP 设为 0。

6. 答：该指令用于交换 sscratch 和 sp 的值，交换后 sp 指向 kernel stack 的栈顶，sscratch 指向 user stack 的栈顶

7. 答：由 U mode 切换到 S mode 并未在 trap.S 中发生

# 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

无

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

- [rust 中指针的使用](https://web.mit.edu/rust-lang_v1.25/arch/amd64_ubuntu1404/share/doc/rust/html/std/primitive.pointer.html)
- rustwiki
- RISC-V-Reader-Chinese-v1

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。


