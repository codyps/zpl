# Zebra firmware CLI

`zebra-firmware` applies a local, extracted Zebra firmware `.zpl` file over raw
TCP (Ethernet or Wi-Fi). It requires SGD `device.product_name`, `device.unique_id`,
and `appl.name` support. Bounded SGD reads are shared with the proxy through
`zebra-sgd`. USB, serial, firmware discovery/download, password entry,
and changing printer security settings are not implemented.

Install from this checkout:

```sh
cargo install --path zebra-firmware --locked
zebra-firmware --printer 192.0.2.10:9100 inspect
```

Use the printer's actual IP and raw port, normally 9100. IPv6 endpoints use
`[address]:9100`. Hostnames are not accepted, so one invocation stays pinned to
one address. `inspect` reads and prints the model, serial, and installed firmware.

1. Obtain the firmware for the exact printer model from [Zebra printer support](https://www.zebra.com/us/en/support-downloads/printers.html).
   Extract the `.zpl` payload from any ZIP archive. Read its release notes for
   supported hardware, intermediate upgrade requirements, and downgrade restrictions.
2. Run `inspect` and compare its model and serial with the physical printer.
3. Preview using those exact values and the expected firmware version:

   ```sh
   zebra-firmware --printer 192.0.2.10:9100 apply ./firmware.zpl \
     --model ZD621 --serial YOUR_SERIAL --version EXPECTED_VERSION
   ```

   Preview contacts the printer for identity queries but sends no firmware. It
   prints the file size and SHA-256. Supply `--sha256 HEX` to require a known digest
   of the extracted file. Model, serial, and version comparisons are exact.
4. Stop other print jobs, maintain stable power, and repeat the same command with
   `--execute` to send the update. The printer may reboot during installation.
   Do not interrupt its power while updating.

The CLI holds a bounded snapshot (maximum 128 MiB) of the file, then checks printer
identity and uploads those exact bytes on the same connection. It rejects empty
files and ZIP archives, but does **not** authenticate firmware, parse its download
header, or determine model compatibility. The model option checks the destination,
not the file. Use only the appropriate original Zebra payload. A matching SHA-256
checks file integrity, not publisher authenticity.

If the printer already reports the requested version, nothing is sent. Otherwise,
transfer has a 120-second deadline. After sending, the CLI polls the same endpoint
for up to 600 seconds (`--wait-seconds 1..3600` changes this). Success requires the
same model and serial to report the exact requested version; it does not establish
print quality or full operational readiness.

Upload failures are never retried automatically. A disconnect or verification
timeout exits nonzero and leaves installation status uncertain: inspect the
printer before deciding what to do next. A printer that changes IP after reboot
cannot be verified at the old endpoint. Disabled raw TCP or restricted SGD access
causes the command to fail; the CLI does not bypass those restrictions.

## Protocol references

- Zebra [Uploading the Latest Firmware](https://docs.zebra.com/us/en/printers/industrial/zt411-zt421-industrial-printer-user-guide/c-zt4x1-setup/r-upgrading-the-printer-firmware/t-uploading-the-latest-firmware.html)
  documents uploading the model-specific `.zpl` firmware file.
- Zebra [ConnectionBuilder](https://techdocs.zebra.com/link-os/2-14/pc/content/com/zebra/sdk/comm/connectionbuilder)
  documents raw TCP connections and the default port 9100.
- Zebra [Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
  `getvar`, `device.product_name`, `device.unique_id`, and `appl.name` sections,
  defines the identity queries.

Local regression tests use a loopback printer simulator and never send firmware
to a physical device:

```sh
cargo test -p zebra-firmware --locked
```
