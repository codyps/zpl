const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { test } = require('node:test');
const vm = require('node:vm');

const html = readFileSync(`${__dirname}/../assets/index.html`, 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];

function page(fetch) {
    let submit;
    const element = (value = '') => ({ value, textContent: '', hidden: true,
        addEventListener() {}, removeAttribute(key) { delete this[key]; },
        replaceChildren(...options) { this.value = options[0]?.value ?? ''; },
    });
    const elements = {
        zpl: element('^XA^FDPreview^FS^XZ'),
        'preview-form': { addEventListener(event, handler) { submit = handler; } },
        printer: element(), preview: element(), image: element(),
        status: element(), error: element(),
    };
    const blobs = [];
    const revoked = [];
    vm.runInNewContext(script, {
        document: { getElementById: id => elements[id] },
        fetch: (url, options) => url === 'api/printers'
            ? Promise.resolve(new Response(JSON.stringify(['TestPrinter'])))
            : fetch(url, options),
        Option: function(text, value) { this.text = text; this.value = value; },
        URL: {
            createObjectURL(blob) { blobs.push(blob); return `blob:${blobs.length}`; },
            revokeObjectURL(url) { revoked.push(url); },
        },
    });
    return { click: () => submit({ preventDefault() {} }), elements, blobs, revoked, ready: () => new Promise(resolve => setImmediate(resolve)) };
}

test('preview posts the textarea to the server route and displays PNG bytes', async () => {
    const png = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
    const view = page(async (url, options) => {
        assert.equal(url, 'api/printers/TestPrinter/preview');
        assert.equal(options.method, 'POST');
        assert.equal(options.headers.Accept, 'image/png');
        assert.equal(options.headers['Content-Type'], 'application/json');
        assert.deepEqual(JSON.parse(options.body), { zpl: view.elements.zpl.value });
        assert.equal(view.elements.preview.disabled, true);
        return new Response(png, { headers: { 'Content-Type': 'image/png' } });
    });
    await view.ready();
    await view.click();
    assert.equal(view.elements.error.textContent, '');
    assert.equal(view.elements.image.src, 'blob:1');
    assert.deepEqual(Buffer.from(await view.blobs[0].arrayBuffer()), png);
    view.elements.zpl.value = '^XA^FDSecond^FS^XZ';
    await view.ready();
    await view.click();
    assert.equal(view.elements.image.src, 'blob:2');
    assert.deepEqual(view.revoked, ['blob:1']);
    assert.equal(view.elements.preview.disabled, false);
});

test('HTTP and network failures are visible and allow retry', async () => {
    let attempts = 0;
    const view = page(async () => {
        if (++attempts === 1) return new Response('printer unavailable', { status: 500 });
        throw new Error('Network unavailable');
    });
    await view.ready();
    await view.click();
    assert.equal(view.elements.error.textContent, 'printer unavailable');
    assert.equal(view.elements.preview.disabled, false);
    await view.ready();
    await view.click();
    assert.equal(view.elements.error.textContent, 'Network unavailable');
    assert.equal(view.elements.preview.disabled, false);
    assert.equal(view.blobs.length, 0);
});
