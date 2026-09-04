<script>
  // The flag of the European Union (R2, PLAN-RETOURS-11) — reduced
  // official geometry: 3:2 field (30×20 grid), twelve five-pointed
  // stars point up, centered on a circle of radius h/3, each star
  // inscribed in a circle of radius h/9. The points are BAKED IN
  // (review 2026-08-28): the image is strictly constant, like the
  // glyph registry's traces — and the System's mockup
  // (system.dc.html, § Settings) carries the same coordinates
  // verbatim, the brand's precedent. Colors FIXED outside themes
  // (the tile brand's regime, W-D3): these are the flag's colors,
  // not the product's. Outside the glyph registry: the registry is
  // monochrome `currentColor` — a two-color flag has no place there
  // (precedent: BRAND already lives outside the inventory).
  let { height = 14 } = $props();
</script>

<svg
  data-name="drapeau-ue"
  viewBox="0 0 30 20"
  width={(height * 30) / 20}
  height={height}
  aria-hidden="true"
>
  <rect width="30" height="20" fill="#003399" />
  <g fill="#FFCC00">
    <polygon points="15.00,1.11 15.50,2.65 17.11,2.65 15.81,3.60 16.31,5.13 15.00,4.18 13.69,5.13 14.19,3.60 12.89,2.65 14.50,2.65" />
    <polygon points="18.33,2.00 18.83,3.54 20.45,3.54 19.14,4.49 19.64,6.02 18.33,5.08 17.03,6.02 17.53,4.49 16.22,3.54 17.83,3.54" />
    <polygon points="20.77,4.44 21.27,5.98 22.89,5.98 21.58,6.93 22.08,8.46 20.77,7.52 19.47,8.46 19.97,6.93 18.66,5.98 20.27,5.98" />
    <polygon points="21.67,7.78 22.17,9.31 23.78,9.31 22.47,10.26 22.97,11.80 21.67,10.85 20.36,11.80 20.86,10.26 19.55,9.31 21.17,9.31" />
    <polygon points="20.77,11.11 21.27,12.65 22.89,12.65 21.58,13.60 22.08,15.13 20.77,14.18 19.47,15.13 19.97,13.60 18.66,12.65 20.27,12.65" />
    <polygon points="18.33,13.55 18.83,15.09 20.45,15.09 19.14,16.04 19.64,17.57 18.33,16.62 17.03,17.57 17.53,16.04 16.22,15.09 17.83,15.09" />
    <polygon points="15.00,14.44 15.50,15.98 17.11,15.98 15.81,16.93 16.31,18.46 15.00,17.52 13.69,18.46 14.19,16.93 12.89,15.98 14.50,15.98" />
    <polygon points="11.67,13.55 12.17,15.09 13.78,15.09 12.47,16.04 12.97,17.57 11.67,16.62 10.36,17.57 10.86,16.04 9.55,15.09 11.17,15.09" />
    <polygon points="9.23,11.11 9.73,12.65 11.34,12.65 10.03,13.60 10.53,15.13 9.23,14.18 7.92,15.13 8.42,13.60 7.11,12.65 8.73,12.65" />
    <polygon points="8.33,7.78 8.83,9.31 10.45,9.31 9.14,10.26 9.64,11.80 8.33,10.85 7.03,11.80 7.53,10.26 6.22,9.31 7.83,9.31" />
    <polygon points="9.23,4.44 9.73,5.98 11.34,5.98 10.03,6.93 10.53,8.46 9.23,7.52 7.92,8.46 8.42,6.93 7.11,5.98 8.73,5.98" />
    <polygon points="11.67,2.00 12.17,3.54 13.78,3.54 12.47,4.49 12.97,6.02 11.67,5.08 10.36,6.02 10.86,4.49 9.55,3.54 11.17,3.54" />
  </g>
</svg>
