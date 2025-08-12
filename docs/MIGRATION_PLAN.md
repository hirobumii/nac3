# NAC3 to RTMQv2 Backend Migration: Analysis Plan

This document outlines the plan for analyzing the feasibility of migrating the NAC3 compiler's backend from LLVM to the custom RTMQv2 ISA.

The primary source for this analysis will be the `RTMQv2 Reference #1 - Instruction Set & Core Architecture` manual.

## Analysis Steps

The analysis will be conducted in three main phases:

### 1. Feature Mapping and Gap Analysis

This phase focuses on understanding how Python language features can be represented in the RTMQv2 ISA.

*   **Data Types**: Map Python's data types (integers, floats, strings, bools, lists, dicts, custom objects) to the 32-bit integer-based RTMQv2 architecture. Pay special attention to the lack of native floating-point support.
*   **Control Flow**: Determine how to implement `if/else` statements, `for` and `while` loops, and `try/except` blocks using RTMQv2's `PTR` register and conditional execution.
*   **Functions and Calling Convention**: Design a calling convention for passing arguments and returning values, using the `LNK` (link register) and `STK` (stack pointer) for the Tightly-Coupled Stack (TCS).
*   **Memory Model**: This is a critical area. Analyze how to manage memory for Python objects. The TCS seems to serve as a register window and stack, but general-purpose heap allocation, which is essential for most Python programs, is not explicitly described in the manual. This may be the biggest challenge.
*   **Object-Oriented Features**: Investigate how classes, methods, and inheritance could be implemented on top of the designed memory model.

### 2. Identifying Challenges and Limitations

Based on the feature mapping, I will create a list of key challenges and limitations. This will likely include:

*   **Floating-Point Arithmetic**: The ISA does not seem to support floating-point numbers. This would be a major limitation for general-purpose Python code.
*   **Dynamic Memory Allocation**: The lack of a clear heap or general-purpose memory access instructions is a significant hurdle for supporting complex data structures.
*   **Python Standard Library**: A large portion of the Python standard library will likely be incompatible without significant porting effort.
*   **Garbage Collection**: Implementing an automatic garbage collector on this architecture would be a substantial project in itself.

### 3. High-Level Backend Strategy

Finally, I will outline a high-level strategy for implementing the new backend within the `nac3core` crate.

*   **Modular Integration**: Propose how to add the new RTMQv2 backend alongside the existing LLVM backend, potentially using a compile-time flag to switch between them.
*   **Code Generation Architecture**: Describe the new `rtmq_codegen` module, which would traverse the NAC3 AST (or an intermediate representation) and emit RTMQv2 assembly.
*   **Development Roadmap**: Suggest a phased implementation plan, starting with a minimal subset of Python (e.g., integer arithmetic and static control flow) and gradually adding more features.

The outcome of this analysis will be a report that details the feasibility of the project, a proposed Python subset that could be supported, and a plan for the implementation work.
