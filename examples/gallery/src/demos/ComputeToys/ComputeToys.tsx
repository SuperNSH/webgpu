// apps/example/src/ComputeToys/ComputeToys.tsx: compute.toys shader 2349,
// fetched at run time from compute.toys as the reference does.

import { Text } from "nsh";
import { ComputeToy, useComputeToy } from "./ComputeToy";

export const ComputeToys = () => {
  //1806
  const toy = useComputeToy(2349);
  return (
    <If when={toy()} else={<Text class="p-4 text-sm text-muted-foreground">Loading compute.toys #2349…</Text>}>
      {(loaded) => <ComputeToy toy={loaded()} />}
    </If>
  );
};
