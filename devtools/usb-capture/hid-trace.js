// Frida script: logs the HID traffic between an app (e.g. miniDSP Device Console) and a miniDSP,
// in the format read by `minidsp-devtools decode`:
//
//   Sent: <hex frame>      a report the app sent to the device
//   Recv: <hex frame>      a report the device sent back
//   # <text>               a comment, e.g. a marker added with mark("...")
//
// Usage (see README.md in this directory):
//   sudo frida -f "<app>/Contents/MacOS/<binary>" -l hid-trace.js -o capture.log
// then, in the Frida prompt, label what you do:
//   mark("set OUT 1 PEQ band 1 gain to -3 dB")

'use strict';

/** miniDSP's USB vendor id. Set to -1 to log every HID device the app talks to. */
const VENDOR_FILTER = 0x2752;

const t0 = Date.now();
const iokit = Process.getModuleByName('IOKit');
const cf = Process.getModuleByName('CoreFoundation');
const fn = (module, name, ret, args) => new NativeFunction(module.getExportByName(name), ret, args);

// ---- Identifying the device behind an IOHIDDeviceRef ----

const CFStringCreateWithCString = fn(cf, 'CFStringCreateWithCString', 'pointer', ['pointer', 'pointer', 'uint32']);
const CFNumberGetValue = fn(cf, 'CFNumberGetValue', 'bool', ['pointer', 'int', 'pointer']);
const IOHIDDeviceGetProperty = fn(iokit, 'IOHIDDeviceGetProperty', 'pointer', ['pointer', 'pointer']);
const kCFStringEncodingUTF8 = 0x08000100;
const kCFNumberSInt32Type = 3;
const vendorKey = CFStringCreateWithCString(NULL, Memory.allocUtf8String('VendorID'), kCFStringEncodingUTF8);
const vendors = new Map();

function vendorOf(device) {
  const key = device.toString();
  if (!vendors.has(key)) {
    let vendor = -1;
    try {
      const number = IOHIDDeviceGetProperty(device, vendorKey);
      if (!number.isNull()) {
        const out = Memory.alloc(4);
        if (CFNumberGetValue(number, kCFNumberSInt32Type, out)) vendor = out.readS32();
      }
    } catch (e) {
      // Unknown device, keep -1
    }
    vendors.set(key, vendor);
  }
  return vendors.get(key);
}

const wanted = (device) => VENDOR_FILTER < 0 || device.isNull() || vendorOf(device) === VENDOR_FILTER;

// ---- Logging ----

const hex = (bytes) => Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');

/**
 * Logs a report as a miniDSP frame: [length, payload..., checksum]. A leading report id of 0 is
 * dropped (a frame's length byte is never 0), and the padding after the frame is trimmed.
 */
function log(direction, ptr, length) {
  if (ptr.isNull() || length <= 0) return;
  let bytes = new Uint8Array(ptr.readByteArray(Math.min(length, 1024)));
  if (bytes.length > 1 && bytes[0] === 0) bytes = bytes.subarray(1);
  const frameLength = bytes[0] + 1; // length byte counts itself and the payload, then the checksum
  if (frameLength > 1 && frameLength <= bytes.length) bytes = bytes.subarray(0, frameLength);
  console.log(`${direction}: ${hex(bytes)}`);
}

/** Adds a marker to the capture, e.g. mark("changed OUT 1 gain to -6 dB") */
globalThis.mark = function (text) {
  console.log(`# ${((Date.now() - t0) / 1000).toFixed(3)}s ${text}`);
  return 'marked';
};

// ---- Hooks ----

function hook(name, callbacks) {
  const address = iokit.findExportByName(name);
  if (!address) {
    console.log(`# ${name} not found, not hooked`);
    return;
  }
  Interceptor.attach(address, callbacks);
}

// Reports sent by the app
hook('IOHIDDeviceSetReport', {
  // (device, type, reportID, report, reportLength)
  onEnter(args) {
    if (wanted(args[0])) log('Sent', args[3], args[4].toInt32());
  },
});
hook('IOHIDDeviceSetReportWithCallback', {
  // (device, type, reportID, report, reportLength, timeout, callback, context)
  onEnter(args) {
    if (wanted(args[0])) log('Sent', args[3], args[4].toInt32());
  },
});

// Reports read synchronously by the app
hook('IOHIDDeviceGetReport', {
  // (device, type, reportID, report, *reportLength)
  onEnter(args) {
    this.device = args[0];
    this.report = args[3];
    this.length = args[4];
  },
  onLeave(retval) {
    if (retval.toInt32() !== 0 || !wanted(this.device)) return;
    log('Recv', this.report, this.length.readS64().toNumber());
  },
});

// Reports the device sends on its own: wrap the app's input callbacks
const hookedCallbacks = new Set();
function hookInputCallback(callback) {
  const key = callback.toString();
  if (callback.isNull() || hookedCallbacks.has(key)) return;
  hookedCallbacks.add(key);
  Interceptor.attach(callback, {
    // (context, result, sender, type, reportID, report, reportLength[, timeStamp])
    onEnter(args) {
      if (wanted(args[2])) log('Recv', args[5], args[6].toInt32());
    },
  });
}
for (const name of ['IOHIDDeviceRegisterInputReportCallback', 'IOHIDDeviceRegisterInputReportWithTimeStampCallback']) {
  hook(name, {
    // (device, report, reportLength, callback, context)
    onEnter(args) {
      hookInputCallback(args[3]);
    },
  });
}

console.log(`# capture started ${new Date().toISOString()}, filtering vendor ${VENDOR_FILTER < 0 ? 'none' : '0x' + VENDOR_FILTER.toString(16)}`);
