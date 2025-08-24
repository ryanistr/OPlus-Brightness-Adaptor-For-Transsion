### OPlus-Brightness-Adaptor-For-Transsion
<<<<<<< HEAD
=======
A Quick Fix and simple for OPlus/Oppo/Realme Ports on Transsion Devices
by @rianixia on telegram, do credit when use.
>>>>>>> 08bd33247399b4d298b30fb4c791dbcc4586e281

A quick fix and simple solution for OPlus/Oppo/Realme ports on Transsion devices.

*By @rianixia on Telegram. Please give credit when using.*

<<<<<<< HEAD
The code is open source because I decided not to gatekeep it. If you know a better logic or method, please share! I'm always listening to feedback—even if my responses are rough. Thank you\~
=======
### How to Use
*Note : This was originally made for OS15 ROMs but if you want to try on OS14 or under i recommend you to add this prop*
```persist.sys.rianixia.brightness.isfloat=true```
>>>>>>> 08bd33247399b4d298b30fb4c791dbcc4586e281

---

## ⚠️ Important

**DO NOT DELETE your stock Transsion light HALs.**
Just add this alongside them.

---

## How to Use

> **Note:** Originally made for OS15 ROMs. For OS14 or lower, it's recommended to add this property:

```bash
persist.sys.rianixia.brightness.isfloat=true
```

### Step 1: Copy Files to Vendor

File structure should look like this:

```
/vendor/bin/hw/vendor.xia.display.adaptor-V4@1.0-service
/vendor/etc/init/init.xia.display.adaptor.rc
```

### Step 2: Set Binary File Context to mtk_light on fs_context

<<<<<<< HEAD
```bash
u:object_r:mtk_hal_light_exec:s0
```
=======
# Final : Add this prop for debugging or completely skip if youre certain
```persist.sys.rianixia.display-debug=true```
>>>>>>> 08bd33247399b4d298b30fb4c791dbcc4586e281

### Step 3: Add Debug Prop (Optional)

```bash
persist.sys.rianixia.display-debug=true
```

---

## SEPolicy

None.
Address manually or use permissive mode. Each device may have different sysfs labels.

---

## Display Types

### IPS LCD Displays

Should work as-is. If not, refer to AMOLED method.

### AMOLED Displays

Make sure to check `my_product` and remove props containing: `vrr`, `brightness`, `silky`, `underscreen`. Then add the following props in vendor properties:

```bash
# Brightness Props for AMOLED
persist.sys.tran.brightness.gammalinear.convert=1
ro.vendor.transsion.backlight_hal.optimization=1
ro.transsion.backlight.level=-1
ro.transsion.physical.backlight.optimization=1
```
<<<<<<< HEAD

---

### License

Open source. Use responsibly and give credit.

=======

>>>>>>> 08bd33247399b4d298b30fb4c791dbcc4586e281
