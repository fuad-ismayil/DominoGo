"""Package the current Dioxus entrypoint, not stale historical build artifacts."""
from pathlib import Path
import re
import shutil

root = Path(__file__).resolve().parent.parent
assets = (root / "android-host/app/src/main/assets").resolve()
for game,source,folder in [
    ("DominoGo",root/"rust/target/dx/domino-client/release/web/public","web"),
    ("UnoGo",root/"unogo/target/dx/uno-client/release/web/public","web-unogo"),
]:
    if not (source/"index.html").is_file():
        raise RuntimeError(f"Build the {game} release browser client first")
    target=(assets/folder).resolve()
    # Purge only the verified generated browser directory, never app data.
    if target.parent!=assets or not target.is_relative_to(root.resolve()):
        raise RuntimeError("Unsafe generated asset path")
    if target.exists():shutil.rmtree(target)
    target.mkdir(parents=True)
    for item in source.iterdir():
        if item.name!="assets":
            if item.is_dir():shutil.copytree(item,target/item.name)
            else:shutil.copy2(item,target/item.name)
    pending=[source/name.lstrip("/.") for name in re.findall(r'["\'](/[^"\']+\.js)["\']',(source/"index.html").read_text())]
    copied=set()
    while pending:
        file=pending.pop()
        if file in copied:continue
        if not file.is_file() or source.resolve() not in file.resolve().parents:raise RuntimeError(f"Missing or unsafe asset: {file}")
        relative=file.relative_to(source);(target/relative).parent.mkdir(parents=True,exist_ok=True);shutil.copy2(file,target/relative);copied.add(file)
        if file.suffix==".js":
            for name in re.findall(r'["\'](/[^"\']+\.(?:js|wasm))["\']',file.read_text()):pending.append(source/name.lstrip("/."))
    print(f"Prepared {game}: {len(copied)} current browser bundles")
