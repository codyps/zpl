// Module workers keep synchronous Wasm off the UI thread.
// https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers
import { withSvgMetadata } from './svg-metadata.mjs';
import { withPngMetadata } from './png-metadata.mjs';
import init, { render_preview, library_version } from './pkg/zpl_wasm.js';
const ready = init();
self.onmessage = async ({ data }) => {
  let result;
  try {
    await ready;
    result = render_preview(data.source, data.width, data.height, data.dpi, data.label);
    const metadata = {
      source: data.source,
      version: library_version(),
      print: data.printId,
      label: data.label + 1,
      labels: result.labels,
      width: data.width,
      height: data.height,
      dpi: data.dpi,
      profile: 'ZD621_203_DPI',
    };
    const png = withPngMetadata(result.png(), metadata);
    const svg = withSvgMetadata(result.svg(), metadata);
    const pdf = result.pdf();
    self.postMessage({ png, svg, pdf, width: result.width, height: result.height,
      labels: result.labels, warnings: result.warnings() }, [png.buffer, svg.buffer, pdf.buffer]);
  } catch (error) {
    self.postMessage({ error: error instanceof Error ? error.message : String(error) });
  } finally {
    result?.free();
  }
};
