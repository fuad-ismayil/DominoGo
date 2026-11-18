"""Static APK checks; does not install, contact or claim to test a phone."""
import hashlib,json,os,re,struct,subprocess,sys,zipfile
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
apk=Path(sys.argv[1] if len(sys.argv)>1 else ROOT/"android-host/dist/GoHost.apk").resolve()
sdk=Path(os.environ.get("ANDROID_HOME",os.environ.get("ANDROID_SDK_ROOT","")))
tools=sdk/"build-tools/35.0.1"
def command(args):return subprocess.check_output([str(a) for a in args],text=True,stderr=subprocess.STDOUT)
def signer(path):
    output=command([tools/"apksigner.bat","verify","--print-certs",path])
    return re.search(r"certificate SHA-256 digest: ([a-f0-9]+)",output).group(1)

gradle=(ROOT/"android-host/app/build.gradle").read_text(encoding="utf8")
rules_version=re.search(r'pub const RULES_VERSION: &str = "([^"]+)"',(ROOT/"unogo/crates/uno-core/src/lib.rs").read_text(encoding="utf8")).group(1)
assert rules_version in gradle,"Android metadata must match the engine rules"
version=int(re.search(r"versionCode (\d+)",gradle).group(1));name=re.search(r"versionName '([^']+)'",gradle).group(1)
badging=command([tools/"aapt.exe","dump","badging",apk])
assert "name='games.dominogo.host'" in badging and f"versionCode='{version}'" in badging and f"versionName='{name}'" in badging
assert "application-label:'Go Host'" in badging
certificate=signer(apk)
reference=ROOT/"android-host/dist/archive/DominoGo-Host-v1.5.apk"
if reference.exists():assert certificate==signer(reference),"Signing identity differs from the existing APK"
with zipfile.ZipFile(apk) as z:
    entries=set(z.namelist())
    assert not any(p.lower().endswith((".sqlite",".sqlite3",".db",".keystore",".jks")) for p in entries),"Private data must never ship in assets"
    assert not any("credentials" in p.lower() or p.lower().endswith(".pem") for p in entries)
    for folder,title in [("web","DominoGo"),("web-unogo","UnoGo")]:
        prefix=f"assets/{folder}/";index=z.read(prefix+"index.html").decode()
        assert f"<title>{title}</title>" in index
        for js in re.findall(r'<script[^>]+src="([^"]+)"',index):assert prefix+js.lstrip("/.") in entries
        assert prefix+"fonts/PlusJakartaSans.ttf" in entries
        if folder=="web-unogo":
            manifest=json.loads(z.read(prefix+"manifest.webmanifest"))
            assert manifest["name"]=="UnoGo" and manifest["display"]=="standalone"
            assert prefix+"brand/unogo-mark-v2.svg" in entries
            for icon in manifest["icons"]:
                png=z.read(prefix+icon["src"].lstrip("/"))
                assert png[:8]==b"\x89PNG\r\n\x1a\n"
                width,height=struct.unpack_from(">II",png,16)
                assert f"{width}x{height}"==icon["sizes"]
            assert prefix+"brand/apple-touch-icon-v2.png" in entries
    for library in ["libdomino_server.so","libcloudflared.so"]:
        data=z.read("lib/arm64-v8a/"+library)
        assert data[:6]==b"\x7fELF\x02\x01" and struct.unpack_from("<H",data,18)[0]==183
        offset=struct.unpack_from("<Q",data,32)[0];size,count=struct.unpack_from("<HH",data,54)
        for i in range(count):
            ph=offset+i*size
            if struct.unpack_from("<I",data,ph)[0]==1:
                file_offset,address=struct.unpack_from("<QQ",data,ph+8)
                assert struct.unpack_from("<Q",data,ph+48)[0]>=16384 and file_offset%16384==address%16384,library+" has incompatible page alignment"
        if library=="libdomino_server.so":
            assert rules_version.encode() in data,"Android native engine is stale; rebuild it before packaging"
            for symbol in [b"nativeRun",b"nativeSetEnabled",b"nativeStatus",b"nativeStop"]:
                assert b"Java_games_dominogo_host_HostService_"+symbol in data,"Missing shared-host JNI entry"
apk_hash=hashlib.sha256(apk.read_bytes()).hexdigest()
device_check_path=apk.parent/"GoHost.device-check.json"
device_check=json.loads(device_check_path.read_text()) if device_check_path.exists() else {}
phone_tested=device_check.get("apkSha256")==apk_hash and device_check.get("result")=="passed"
info={"file":apk.name,"version":name,"versionCode":version,"package":"games.dominogo.host","certificateSha256":certificate,"sha256":apk_hash,"bytes":apk.stat().st_size,"games":["dominogo","unogo"],"unoRulesVersion":rules_version,"schemaFamily":"shared-go-host-v1","abi":"arm64-v8a","elfPageAlignment":16384,"phoneTested":phone_tested,"hardwareAcceptanceComplete":False,"published":False}
(apk.parent/"GoHost.build.json").write_text(json.dumps(info,indent=2)+"\n",encoding="utf8")
print(json.dumps(info,indent=2))
