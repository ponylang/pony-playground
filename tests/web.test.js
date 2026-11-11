const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(
    path.join(__dirname, '../static/web.js'), 'utf8');

function loadClient(fetch) {
    assert.equal(source.split('}());').length, 2);
    const context = vm.createContext({
        fetch, AbortSignal, DOMException,
        addEventListener() {},
    });
    vm.runInContext(source.replace('}());',
        'globalThis.requestTest = {send, set_result}; }());'), context);
    context.requestTest.set_result.editor = {resize() {}};
    return {
        send: context.requestTest.send,
        result: {parentNode: {dataset: {}}, innerHTML: ''},
        button: {disabled: false},
    };
}

test('request stays pending until its JSON body is complete', async () => {
    let bodyStarted;
    const jsonStarted = new Promise(resolve => {bodyStarted = resolve;});
    let completeBody;
    const body = new Promise(resolve => {completeBody = resolve;});
    let started = false;
    const client = loadClient(async () => ({
        status: 200,
        json() {started = true; bodyStarted(); return body;},
    }));
    const payload = {success: true, stdout: '42\n'};
    let calls = 0;
    const pending = client.send('/evaluate.json', {}, value => {
        calls++;
        assert.equal(value, payload);
    }, client.button, 'Running', client.result);
    assert.equal(client.button.disabled, true);
    await Promise.race([jsonStarted, pending]);
    assert.equal(started, true);
    assert.equal(client.button.disabled, true);
    assert.equal(calls, 0);
    completeBody(payload);
    await pending;
    assert.equal(calls, 1);
    assert.equal(client.button.disabled, false);
});

for (const name of ['TimeoutError', 'AbortError', 'TypeError']) {
    test(`failed fetch handles ${name} and restores the button`, async () => {
        const error = name === 'TypeError'
            ? new TypeError('network unavailable')
            : new DOMException('deadline reached', name);
        const client = loadClient(async () => {
            throw error;
        });
        let calls = 0;
        const pending = client.send('/evaluate.json', {}, () => {calls++;},
            client.button, 'Running', client.result);
        assert.equal(client.button.disabled, true);
        await pending;
        assert.equal(calls, 0);
        assert.equal(client.button.disabled, false);
        if (name === 'TypeError') {
            assert.match(client.result.innerHTML, /network unavailable/);
        } else {
            assert.match(client.result.innerHTML, /Request timed out/);
        }
    });
}

for (const name of ['TimeoutError', 'AbortError', 'SyntaxError']) {
    test(`failed response body handles ${name} without a callback`, async () => {
        let bodyStarted;
        const jsonStarted = new Promise(resolve => {bodyStarted = resolve;});
        let failBody;
        const body = new Promise((resolve, reject) => {failBody = reject;});
        let started = false;
        const client = loadClient(async () => ({
            status: 200,
            json() {started = true; bodyStarted(); return body;},
        }));
        let calls = 0;
        const pending = client.send('/compile.json', {}, () => {calls++;},
            client.button, 'Compiling', client.result);
        assert.equal(client.button.disabled, true);
        await Promise.race([jsonStarted, pending]);
        assert.equal(started, true);
        assert.equal(client.button.disabled, true);
        assert.equal(calls, 0);
        failBody(name === 'SyntaxError'
            ? new SyntaxError('<invalid JSON>')
            : new DOMException('body deadline reached', name));
        await pending;
        assert.equal(calls, 0);
        assert.equal(client.button.disabled, false);
        if (name === 'SyntaxError') {
            assert.match(client.result.innerHTML, /&lt;invalid JSON&gt;/);
        } else {
            assert.match(client.result.innerHTML, /Request timed out/);
        }
    });
}

for (const status of [0, 201, 500]) {
    test(`HTTP status ${status} is reported without reading JSON`, async () => {
        let reads = 0;
        const client = loadClient(async () => ({
            status,
            ok: status === 201,
            json() {reads++; throw new SyntaxError('unexpected body read');},
        }));
        let calls = 0;
        const pending = client.send('/evaluate.json', {}, () => {calls++;},
            client.button, 'Running', client.result);
        assert.equal(client.button.disabled, true);
        await pending;
        assert.equal(reads, 0);
        assert.equal(calls, 0);
        assert.equal(client.button.disabled, false);
        if (status === 0) {
            assert.match(client.result.innerHTML, /Connection failure/);
            assert.match(client.result.innerHTML, /Are you connected to the Internet\?/);
        } else {
            assert.match(client.result.innerHTML, new RegExp(`status code ${status}\\.`));
        }
    });
}
