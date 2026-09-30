// SVG metadata is non-rendering descriptive content (SVG 2 §5.9).
// https://www.w3.org/TR/SVG2/struct.html#MetadataElement
export function withSvgMetadata(svg, metadata) {
  const source = new TextDecoder().decode(svg);
  // JSON escapes controls; escape XML delimiters and XML 1.0 noncharacters too.
  // Never interpolate the ZPL as markup (including </metadata> or CDATA text).
  const json = JSON.stringify(metadata)
    .replace(/[\ufffe\uffff]/g, c => `\\u${c.charCodeAt(0).toString(16)}`)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const end = source.indexOf('>') + 1; // The renderer starts with the root <svg>.
  return new TextEncoder().encode(`${source.slice(0, end)}<metadata id="zpl-metadata">${json}</metadata>${source.slice(end)}`);
}
