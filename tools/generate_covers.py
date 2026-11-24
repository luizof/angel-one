#!/usr/bin/env python3
"""Ilustra capas com as fotos autorizadas do barco pela Image API do OpenRouter.

Uma submissão por cena, sem retry automático. Usa a chave do serviço privado da
prévia; nunca grava a chave no projeto, no log ou no navegador.
"""
import base64
import json
import os
import pathlib
import subprocess
import sys
import urllib.error
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "app/assets-src/capas"
REF = ROOT / "Fotos do barco para referência"
SCENES = {
    "manha": ("IMG_5850.JPG", "Early morning in Jurerê: peach and pale blue open sky, low sun, wispy mist over Atlantic-forest hills; yacht peacefully at anchor in emerald water."),
    "tarde": ("dji_fly_20230325_114718_208_1679755914429_photo.JPG", "Bright tropical midday in Jurerê: bright pastel-blue open sky, few puffy clouds, turquoise water, white sandy beach; yacht cruising with a delicate wake."),
    "final_de_tarde": ("dji_fly_20260118_195406_995_1768686797348_photo.JPG", "Golden hour in Jurerê: pale apricot and pink open sky, sun low on the mainland hills, sunset glitter on the sea; yacht anchored with warm light on the hull."),
    "noite": ("dji_fly_20230311_104826_139_1678596788401_photo.jpg", "Blue-hour tropical night in Jurerê: an open deep indigo sky with subtle stars and a small moon, beach-house lights, calm dark-blue water; yacht illuminated by warm deck lights and turquoise underwater lights."),
}
PROMPT = """Make a polished HAND-DRAWN 2D travel-poster illustration, NOT a photograph or 3D, of the ACTUAL Angel One yacht in the reference photo.
Its recognizable metallic titanium grey Schaefer flybridge motor-yacht shape, swept bow, graphite window band, upper hardtop, beige decks and proportions must remain accurate.
Location: near Jurerê Internacional, Florianópolis, Brazil, with its low beach houses, green hills, sandy shore and island silhouettes. Scene: {scene}
VERTICAL 3:4 MOBILE HERO COMPOSITION: top 45 percent must be almost empty open smoothly colored sky for readable phone status bar and controls WITHOUT a dark gradient. Boat entirely BELOW 50 percent of the image, centered at 55-78 percent, not touching the left or right edges, water at the bottom.
Refined yet cheerful flat-color gouache, crisp hand-drawn contours, simple layered shapes, subtle paper grain, cohesive tropical palette, evocative 2D illustration. No lettering, text, words, logo, UI, people, frame or watermark."""


def private_key():
    path = pathlib.Path.home() / ".config/angel-one/preview.env"
    for line in path.read_text().splitlines():
        if line.startswith("OPENROUTER_API_KEY="):
            return line.split("=", 1)[1].strip()
    raise RuntimeError("Configure a chave no arquivo privado da prévia")


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in SCENES:
        raise SystemExit("Uso: generate_covers.py manha|tarde|final_de_tarde|noite")
    name = sys.argv[1]
    file, scene = SCENES[name]
    original = REF / file
    prepared = pathlib.Path("/tmp/opencode") / f"angel-reference-{name}.jpg"
    subprocess.run(["magick", str(original), "-auto-orient", "-resize", "1200x1200>", "-strip", "-quality", "82", str(prepared)], check=True)
    reference = "data:image/jpeg;base64," + base64.b64encode(prepared.read_bytes()).decode("ascii")
    req = urllib.request.Request(
        "https://openrouter.ai/api/v1/images",
        data=json.dumps({
            "model": "google/gemini-3.1-flash-image",
            "prompt": PROMPT.format(scene=scene),
            "resolution": "2K", "aspect_ratio": "3:4",
            "input_references": [{"type": "image_url", "image_url": {"url": reference}}],
        }).encode(),
        headers={"Authorization": "Bearer " + private_key(), "Content-Type": "application/json", "X-Title": "Angel One"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=180) as response:
            data = json.load(response)
    except urllib.error.HTTPError as error:
        # A resposta de erro não contém a chave; limitamos a exibição.
        msg = error.read(2048).decode("utf8", "replace")
        raise RuntimeError(f"OpenRouter HTTP {error.code}: {msg[:400]}") from None
    image = data["data"][0]
    mime = image.get("media_type", "image/png")
    if mime not in ("image/png", "image/jpeg", "image/webp"):
        raise RuntimeError("Formato de imagem não aceito: " + mime)
    ext = {"image/png": ".png", "image/jpeg": ".jpg", "image/webp": ".webp"}[mime]
    result = OUT / (name + "-2d" + ext)
    OUT.mkdir(parents=True, exist_ok=True)
    result.write_bytes(base64.b64decode(image["b64_json"]))
    print(f"Ilustração criada em {result} ({len(result.read_bytes())} bytes)")


if __name__ == "__main__":
    main()
