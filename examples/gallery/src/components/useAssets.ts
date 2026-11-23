// apps/example/src/components/useAssets.ts on NSH: the reference resolves a
// `require()`d asset to a URI and fetches it; here the asset is imported
// as an `asset:/` URL (`?url`), which `fetch` reads.

import posx from "../../assets/cubemap/posx.jpg?url";
import negx from "../../assets/cubemap/negx.jpg?url";
import posy from "../../assets/cubemap/posy.jpg?url";
import negy from "../../assets/cubemap/negy.jpg?url";
import posz from "../../assets/cubemap/posz.jpg?url";
import negz from "../../assets/cubemap/negz.jpg?url";
import di3D from "../../assets/Di-3d.png?url";
import f from "../../assets/f.png?url";
import moon from "../../assets/moon.png?url";
import react from "../../assets/react.png?url";
import saturn from "../../assets/saturn.png?url";

/** The example's assets, as `asset:/` URLs. */
export const assets = { di3D, f, moon, react, saturn, cubemap: { posx, negx, posy, negy, posz, negz } };

export const fetchAsset = async (url: string) => {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response;
};

export const decodeImage = async (url: string) => {
  const response = await fetchAsset(url);
  const image = await createImageBitmap(await response.arrayBuffer());
  return image;
};
