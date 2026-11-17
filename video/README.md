# Ролики карточек сайта (Remotion)

Шесть коротких бесшовных роликов для сетки «Что внутри» на `site/`.
Код — `src/cards/*.tsx`, регистрация — `src/Root.tsx`.

```bash
npm i
npx remotion studio        # предпросмотр
bash render-cards.sh       # webm (альфа) + mp4 + png → ../site/assets/cards
```

Фон сцены прозрачный при `--props='{"alpha":true}'` и чёрный для MP4 —
сайт смешивает MP4 через `mix-blend-mode: screen` (Safari не умеет VP9 с альфой).
