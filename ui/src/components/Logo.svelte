<script lang="ts">
  // App logo on a tile: white in the light theme, dark in the dark theme (with the black parts
  // of the logo drawn light so they stay visible).
  let { size = 30 }: { size?: number } = $props();
</script>

<span class="logo" style:width="{size}px" style:height="{size}px" style:border-radius="{Math.round(size * 0.24)}px">
  <img class="for-light" src="/logo.png" alt="" />
  <img class="for-dark" src="/logo-dark.png" alt="" />
</span>

<style>
  .logo {
    flex: none;
    display: inline-grid;
    place-items: center;
    background: #ffffff;
    border: 1px solid var(--border);
    overflow: hidden;
  }

  img {
    width: 88%;
    height: 88%;
    object-fit: contain;
  }

  .for-dark {
    display: none;
  }

  :global(:root[data-theme='dark']) .logo {
    background: #16181d;
  }

  :global(:root[data-theme='dark']) .for-light {
    display: none;
  }

  :global(:root[data-theme='dark']) .for-dark {
    display: block;
  }
</style>
