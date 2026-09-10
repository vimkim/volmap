// Allow a fresh acceptance server alongside an existing interactive preview.
const configuredPort = process.env.VOLMAP_DENSITY_PORT ?? "41742";
if (!/^\d+$/.test(configuredPort) || Number(configuredPort) < 1 || Number(configuredPort) > 65535) {
  throw new Error("VOLMAP_DENSITY_PORT must be an integer from 1 to 65535");
}
export const densityPort = Number(configuredPort);
export const densityUrl = `http://127.0.0.1:${densityPort}`;
