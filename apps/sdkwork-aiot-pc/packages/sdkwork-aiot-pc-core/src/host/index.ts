export interface AiotPcHostCapabilities {
  readonly browser: boolean;
  readonly desktop: boolean;
}

export const aiotPcBrowserHost: AiotPcHostCapabilities = Object.freeze({
  browser: true,
  desktop: false,
});
