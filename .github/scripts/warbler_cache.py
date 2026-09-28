#!/usr/bin/env python3
"""Configure the host-local compiler cache without exposing its credentials."""
import base64
import os
from pathlib import Path
import ssl
import urllib.error
import urllib.request

credential = os.environ.get('WARBLER_CACHE_CREDENTIAL', '')
if not credential:
    print('No cache credential available; building without sccache.')
    raise SystemExit(0)
username, password = credential.split(':', 1)
certificate = Path(__file__).resolve().parent.parent / 'warbler-cache.crt'
context = ssl.create_default_context(cafile=str(certificate))
headers = {'Authorization': 'Basic ' + base64.b64encode(credential.encode()).decode()}
url = 'https://10.77.0.1:9443/zpl/sccache/.sccache_check'
try:
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), context=context, timeout=15):
        pass
except urllib.error.HTTPError as error:
    if error.code != 404:  # An empty cache is valid.
        raise
writer = username == 'writer'
if not writer:
    try:
        urllib.request.urlopen(urllib.request.Request(url, data=b'probe', headers=headers, method='PUT'), context=context, timeout=15)
    except urllib.error.HTTPError as error:
        if error.code not in (401, 403):
            raise
    else:
        raise RuntimeError('Read-only cache credential unexpectedly permits writes')
bundle = Path(os.environ['RUNNER_TEMP']) / 'warbler-cache-ca.pem'
bundle.write_bytes(Path('/etc/ssl/certs/ca-certificates.crt').read_bytes() + certificate.read_bytes())
values = {
    'RUSTC_WRAPPER': 'sccache',
    'CARGO_INCREMENTAL': '0',
    'SCCACHE_WEBDAV_ENDPOINT': 'https://10.77.0.1:9443',
    'SCCACHE_WEBDAV_KEY_PREFIX': 'zpl/sccache/',
    'SCCACHE_WEBDAV_USERNAME': username,
    'SCCACHE_WEBDAV_PASSWORD': password,
    'SCCACHE_WEBDAV_RW_MODE': 'READ_WRITE' if writer else 'READ_ONLY',
    'SSL_CERT_FILE': str(bundle),
}
# GitHub masks the whole credential secret; mask its password component too.
print(f'::add-mask::{password}')
with open(os.environ['GITHUB_ENV'], 'a') as output:
    for key, value in values.items():
        if '\n' in value or '\r' in value:
            raise ValueError('Invalid multiline cache configuration')
        output.write(f'{key}={value}\n')
print(f"Warbler compiler cache verified: {'read/write' if writer else 'read-only (PUT denied)'}, VM {os.uname().nodename}")
