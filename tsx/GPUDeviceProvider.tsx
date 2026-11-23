// <GPUDeviceProvider> and useMainDevice (react-native-webgpu's
// src/GPUDeviceProvider.tsx), for NSH (area: react-parity). React's
// createContext / useContext are NSH's `context()`: the provider requests
// the device with useDevice and renders nothing until it is there, then its
// children inside the context's Provider; useMainDevice reads the nearest
// one, and throws "No DeviceContext found." outside any.

import { context, type JSX } from "nsh";
import { type DeviceState, useDevice } from "./hooks";

interface DeviceContextValue {
  device: GPUDevice | null;
  adapter: GPUAdapter | null;
}

const DeviceContext = context<DeviceContextValue | null>(null);

/** `<GPUDeviceProvider>`'s props, as the reference's `DeviceProviderProps`. */
export interface DeviceProviderProps {
  /** What renders once the device is ready. */
  children?: JSX.Element;
  /** For `navigator.gpu.requestAdapter()`. */
  adapterOptions?: GPURequestAdapterOptions;
  /** For `adapter.requestDevice()`. */
  deviceDescriptor?: GPUDeviceDescriptor;
}

/**
 * Requests one device and shares it with the views inside: renders nothing
 * until the device is ready, then `children`, where `useMainDevice()` gives
 * `{ adapter, device }`.
 */
export const GPUDeviceProvider = (props: DeviceProviderProps): JSX.Element => {
  const state: DeviceState = useDevice(props.adapterOptions, props.deviceDescriptor);
  return (
    <If when={state.device}>
      {(device) => (
        <DeviceContext.Provider value={{ device: device(), adapter: state.adapter }}>
          {props.children}
        </DeviceContext.Provider>
      )}
    </If>
  );
};

/** The `{ adapter, device }` of the nearest `<GPUDeviceProvider>`; throws `No DeviceContext found.` outside one. */
export const useMainDevice = (): DeviceContextValue => {
  const ctx = DeviceContext();
  if (!ctx) {
    throw new Error("No DeviceContext found.");
  }
  return ctx;
};
