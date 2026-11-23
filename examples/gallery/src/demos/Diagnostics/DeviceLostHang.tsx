// apps/example/src/Diagnostics/DeviceLostHang.tsx on NSH: the reference
// forces a loss with Dawn's test-only `forceLossForTesting`, which wgpu does
// not have, so, as the reference does in a build without the helper, the
// screen says it is unavailable.

import { cleanup, signal, Text, View } from "nsh";

export const DeviceLostHang = () => {
  const status = signal("Requesting GPU device…");
  const result = signal<string | null>(null);

  let cancelled = false;
  let resolved = false;
  let lossTimerId: ReturnType<typeof setTimeout> | null = null;
  let watchdogId: ReturnType<typeof setTimeout> | null = null;
  let gpuDevice: GPUDevice | undefined;

  const run = async () => {
    try {
      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) {
        if (!cancelled) {
          result.set("Failed to acquire a GPU adapter.");
        }
        return;
      }

      const device = await adapter.requestDevice({
        label: "device-lost-hang-demo",
      });
      gpuDevice = device;

      if (cancelled) {
        return;
      }

      status.set("device.lost promise captured. Forcing a synthetic device loss in 100 ms…");

      (device.lost as Promise<GPUDeviceLostInfo>)
        .then((info) => {
          resolved = true;
          if (!cancelled) {
            result.set(`device.lost resolved with reason "${info.reason}" and message "${info.message}".`);
          }
        })
        .catch((error) => {
          resolved = true;
          if (!cancelled) {
            result.set(`device.lost rejected: ${error instanceof Error ? error.message : String(error)}`);
          }
        });

      const forceLoss = (device as unknown as { forceLossForTesting?: () => void }).forceLossForTesting;
      if (!forceLoss) {
        result.set(
          "forceLossForTesting is unavailable in this build. Recompile with the test helper to reproduce the issue.",
        );
        return;
      }

      lossTimerId = setTimeout(() => {
        if (cancelled) {
          return;
        }
        status.set("Calling forceLossForTesting – the device lost callback is delivered asynchronously…");
        try {
          forceLoss.call(device);
        } catch {
          // nsh-webgpu keeps the name and throws ERR_WEBGPU_UNSUPPORTED (wgpu
          // cannot lose a device on request): the same as Dawn without the helper.
          resolved = true;
          result.set(
            "forceLossForTesting is unavailable in this build. Recompile with the test helper to reproduce the issue.",
          );
        }
      }, 100);

      watchdogId = setTimeout(() => {
        if (!cancelled && !resolved) {
          result.set(
            "device.lost is still pending several seconds after we forced the loss.\nThis demonstrates that callbacks never fire without additional ProcessEvents pumping.",
          );
        }
      }, 2500);
    } catch (error) {
      if (!cancelled) {
        result.set(`Encountered an error while running the demo: ${error instanceof Error ? error.message : String(error)}`);
      }
    }
  };

  void run();

  cleanup(() => {
    cancelled = true;
    if (lossTimerId !== null) {
      clearTimeout(lossTimerId);
    }
    if (watchdogId !== null) {
      clearTimeout(watchdogId);
    }
    gpuDevice?.destroy();
  });

  return (
    <View class="flex-1 justify-center bg-background p-6">
      <View class="gap-3 rounded-xl bg-card p-5">
        <Text class="text-xl font-semibold text-card-foreground">Device Lost Promise Hang</Text>
        <Text class="text-card-foreground">{status()}</Text>
        <If when={result()}>
          {(text) => (
            <Text
              testID="diag-result"
              class={text().startsWith("device.lost resolved with reason") ? "text-primary" : "text-destructive"}
            >
              {text()}
            </Text>
          )}
        </If>
      </View>
    </View>
  );
};
