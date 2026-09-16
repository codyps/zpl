// Module workers keep synchronous Wasm off the UI thread.
// https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers
import init, { render_preview } from './pkg/zpl_wasm.js';
const ready = init();
self.onmessage = async ({ data }) => {
  let result;
  try {
    await ready;
    result = render_preview(data.source, data.width, data.height, data.dpi, data.label);
    const png = result.png();
    const svg = result.svg();
    self.postMessage({ png, svg, width: result.width, height: result.height,
      labels: result.labels, warnings: result.warnings() }, [png.buffer, svg.buffer]);
  } catch (error) {
    self.postMessage({ error: error instanceof Error ? error.message : String(error) });
  } finally {
    result?.free();
  }
};
