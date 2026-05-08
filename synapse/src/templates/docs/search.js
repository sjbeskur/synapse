const searchInput = document.getElementById('doc-search');
const searchCount = document.getElementById('search-count');
const items = Array.from(document.querySelectorAll('.item'));
const units = Array.from(document.querySelectorAll('.unit'));

function normalize(value) {
  return value.toLowerCase().trim();
}

function applySearch() {
  const query = normalize(searchInput.value);
  let shown = 0;
  for (const item of items) {
    const haystack = normalize(item.dataset.search || item.textContent);
    const match = query === '' || haystack.includes(query);
    item.classList.toggle('hidden', !match);
    if (match) shown += 1;
  }
  for (const unit of units) {
    const hasVisibleItem = Array.from(unit.querySelectorAll('.item')).some((item) => !item.classList.contains('hidden'));
    const unitMatch = query !== '' && normalize(unit.dataset.search || '').includes(query);
    unit.classList.toggle('hidden', query !== '' && !hasVisibleItem && !unitMatch);
  }
  searchCount.textContent = query === ''
    ? 'Showing all declarations.'
    : `Showing ${shown} matching declaration${shown === 1 ? '' : 's'}.`;
}

searchInput.addEventListener('input', applySearch);
