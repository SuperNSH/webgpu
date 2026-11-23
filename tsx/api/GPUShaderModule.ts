// GPUShaderModule (WGSL), GPUCompilationInfo, GPUCompilationMessage.

import { call, WgpuShaderModule } from "../internal/native";
import { HANDLE, INTERNAL, internal, ObjectBase } from "../internal/object";
import { wire } from "../internal/wire";
import type { SpecGPUShaderModule } from "../types/spec";
import type { GPUDevice } from "./GPUDevice";

interface MessageData {
  message: string;
  type: GPUCompilationMessageType;
  lineNum: number;
  linePos: number;
  offset: number;
  length: number;
}

/** A message of the shader compiler. */
export class GPUCompilationMessage {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUCompilationMessage" {
    return "GPUCompilationMessage";
  }
  readonly #data: MessageData;

  constructor(token: unknown, data: MessageData) {
    internal(token);
    this.#data = data;
  }

  /** The text. */
  get message(): string {
    return this.#data.message;
  }

  /** `"error"`, `"warning"` or `"info"`. */
  get type(): GPUCompilationMessageType {
    return this.#data.type;
  }

  /** The line, from 1 (0: none). */
  get lineNum(): number {
    return this.#data.lineNum;
  }

  /** The position in the line, from 1 (0: none). */
  get linePos(): number {
    return this.#data.linePos;
  }

  /** The offset in the code. */
  get offset(): number {
    return this.#data.offset;
  }

  /** The length of the span. */
  get length(): number {
    return this.#data.length;
  }
}

/** What compiling a shader module said. */
export class GPUCompilationInfo {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUCompilationInfo" {
    return "GPUCompilationInfo";
  }
  readonly #messages: readonly GPUCompilationMessage[];

  constructor(token: unknown, messages: readonly GPUCompilationMessage[]) {
    internal(token);
    this.#messages = Object.freeze([...messages]);
  }

  /** The messages. */
  get messages(): readonly GPUCompilationMessage[] {
    return this.#messages;
  }
}

/** Compiled WGSL. */
export class GPUShaderModule extends ObjectBase implements SpecGPUShaderModule {
  /** The class name (react-native-webgpu's `__brand`). */
  get __brand(): "GPUShaderModule" {
    return "GPUShaderModule";
  }

  /** What the compiler said about it. */
  async getCompilationInfo(): Promise<GPUCompilationInfo> {
    const messages = call(() => WgpuShaderModule.compilationInfo(this[HANDLE])) as MessageData[];
    return new GPUCompilationInfo(
      INTERNAL,
      messages.map((m) => new GPUCompilationMessage(INTERNAL, m)),
    );
  }

  get [Symbol.toStringTag](): string {
    return "GPUShaderModule";
  }
}

/** `device.createShaderModule(descriptor)`. */
export function createShaderModule(device: GPUDevice, descriptor: GPUShaderModuleDescriptor): GPUShaderModule {
  const id = call(() => WgpuShaderModule.create(device[HANDLE], wire(descriptor)));
  return new GPUShaderModule(INTERNAL, id, descriptor.label);
}
