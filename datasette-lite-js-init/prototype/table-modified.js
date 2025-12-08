// Modified table.js with SPA/reinit support
// Supports being called multiple times with proper cleanup

var DROPDOWN_HTML = `<div class="dropdown-menu">
<div class="hook"></div>
<ul>
  <li><a class="dropdown-sort-asc" href="#">Sort ascending</a></li>
  <li><a class="dropdown-sort-desc" href="#">Sort descending</a></li>
  <li><a class="dropdown-facet" href="#">Facet by this</a></li>
  <li><a class="dropdown-hide-column" href="#">Hide this column</a></li>
  <li><a class="dropdown-show-all-columns" href="#">Show all columns</a></li>
  <li><a class="dropdown-not-blank" href="#">Show not-blank rows</a></li>
</ul>
<p class="dropdown-column-type"></p>
<p class="dropdown-column-description"></p>
</div>`;

var DROPDOWN_ICON_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="3"></circle>
  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
</svg>`;

// NEW: Track state for cleanup
let currentCleanupFunctions = [];

/**
 * Clean up previous initialization
 * Called before reinitializing to prevent duplicate handlers
 */
function cleanupDatasetteTable() {
  currentCleanupFunctions.forEach(fn => {
    try {
      fn();
    } catch (e) {
      console.warn('Cleanup function failed:', e);
    }
  });
  currentCleanupFunctions = [];

  // Remove existing dropdown menu from body
  const existingMenu = document.querySelector('body > .dropdown-menu');
  if (existingMenu) {
    existingMenu.remove();
  }

  // Remove dropdown icons from all table headers
  document.querySelectorAll('.dropdown-menu-icon').forEach(icon => icon.remove());
}

/**
 * Main initialization function for Datasette Table interactions
 * MODIFIED: Now accepts manager AND root element, supports cleanup
 */
const initDatasetteTable = function (manager, root = document) {
  // Feature detection
  if (!window.URLSearchParams) {
    return;
  }

  // Helper to get URL params (works with both normal and SPA mode)
  function getParams() {
    // In Datasette-lite, the "location" might be in a hash
    let search = location.search;
    if (location.hash && location.hash.includes('?')) {
      search = '?' + location.hash.split('?')[1];
    }
    return new URLSearchParams(search);
  }

  function paramsToUrl(params) {
    var s = params.toString();
    return s ? "?" + s : location.pathname;
  }

  function sortDescUrl(column) {
    var params = getParams();
    params.set("_sort_desc", column);
    params.delete("_sort");
    params.delete("_next");
    return paramsToUrl(params);
  }

  function sortAscUrl(column) {
    var params = getParams();
    params.set("_sort", column);
    params.delete("_sort_desc");
    params.delete("_next");
    return paramsToUrl(params);
  }

  function facetUrl(column) {
    var params = getParams();
    params.append("_facet", column);
    return paramsToUrl(params);
  }

  function hideColumnUrl(column) {
    var params = getParams();
    params.append("_nocol", column);
    return paramsToUrl(params);
  }

  function showAllColumnsUrl() {
    var params = getParams();
    params.delete("_nocol");
    params.delete("_col");
    return paramsToUrl(params);
  }

  function notBlankUrl(column) {
    var params = getParams();
    params.set(`${column}__notblank`, "1");
    return paramsToUrl(params);
  }

  // Create menu element (will be reused)
  var menu = document.createElement("div");
  menu.innerHTML = DROPDOWN_HTML;
  menu = menu.querySelector("*");
  menu.style.position = "absolute";
  menu.style.display = "none";
  document.body.appendChild(menu);

  function closeMenu() {
    menu.style.display = "none";
    menu.classList.remove("anim-scale-in");
  }

  // MODIFIED: Use root element for scoped queries
  const tableWrapper = root.querySelector(manager.selectors.tableWrapper);

  // Set up event listeners with cleanup tracking
  if (tableWrapper) {
    const scrollHandler = () => closeMenu();
    tableWrapper.addEventListener("scroll", scrollHandler);
    currentCleanupFunctions.push(() => tableWrapper.removeEventListener("scroll", scrollHandler));
  }

  const bodyClickHandler = (ev) => {
    var target = ev.target;
    while (target && target != menu) {
      target = target.parentNode;
    }
    if (!target) {
      closeMenu();
    }
  };
  document.body.addEventListener("click", bodyClickHandler);
  currentCleanupFunctions.push(() => document.body.removeEventListener("click", bodyClickHandler));

  function onTableHeaderClick(ev) {
    ev.preventDefault();
    ev.stopPropagation();
    menu.innerHTML = DROPDOWN_HTML;
    var th = ev.target;
    while (th.nodeName != "TH") {
      th = th.parentNode;
    }
    var rect = th.getBoundingClientRect();
    var menuTop = rect.bottom + window.scrollY;
    var menuLeft = rect.left + window.scrollX;
    var column = th.getAttribute("data-column");
    var params = getParams();
    var sort = menu.querySelector("a.dropdown-sort-asc");
    var sortDesc = menu.querySelector("a.dropdown-sort-desc");
    var facetItem = menu.querySelector("a.dropdown-facet");
    var notBlank = menu.querySelector("a.dropdown-not-blank");
    var hideColumn = menu.querySelector("a.dropdown-hide-column");
    var showAllColumns = menu.querySelector("a.dropdown-show-all-columns");

    if (params.get("_sort") == column) {
      sort.parentNode.style.display = "none";
    } else {
      sort.parentNode.style.display = "block";
      sort.setAttribute("href", sortAscUrl(column));
    }
    if (params.get("_sort_desc") == column) {
      sortDesc.parentNode.style.display = "none";
    } else {
      sortDesc.parentNode.style.display = "block";
      sortDesc.setAttribute("href", sortDescUrl(column));
    }
    if (params.get("_nocol") || params.get("_col")) {
      showAllColumns.parentNode.style.display = "block";
      showAllColumns.setAttribute("href", showAllColumnsUrl());
    } else {
      showAllColumns.parentNode.style.display = "none";
    }
    if (th.getAttribute("data-is-pk") != "1") {
      hideColumn.parentNode.style.display = "block";
      hideColumn.setAttribute("href", hideColumnUrl(column));
    } else {
      hideColumn.parentNode.style.display = "none";
    }

    // Check DATASETTE_ALLOW_FACET from the page or use default
    const allowFacet = typeof DATASETTE_ALLOW_FACET !== 'undefined' ? DATASETTE_ALLOW_FACET : true;

    var displayedFacets = Array.from(
      root.querySelectorAll(".facet-info"),
    ).map((el) => el.dataset.column);
    var isFirstColumn =
      th.parentElement.querySelector("th:first-of-type") == th;
    var isSinglePk =
      th.getAttribute("data-is-pk") == "1" &&
      root.querySelectorAll('th[data-is-pk="1"]').length == 1;
    if (
      !allowFacet ||
      isFirstColumn ||
      displayedFacets.includes(column) ||
      isSinglePk
    ) {
      facetItem.parentNode.style.display = "none";
    } else {
      facetItem.parentNode.style.display = "block";
      facetItem.setAttribute("href", facetUrl(column));
    }

    var tdsForThisColumn = Array.from(
      th.closest("table").querySelectorAll("td." + th.className),
    );
    if (
      params.get(`${column}__notblank`) != "1" &&
      tdsForThisColumn.filter((el) => el.innerText.trim() == "").length
    ) {
      notBlank.parentNode.style.display = "block";
      notBlank.setAttribute("href", notBlankUrl(column));
    } else {
      notBlank.parentNode.style.display = "none";
    }

    var columnTypeP = menu.querySelector(".dropdown-column-type");
    var columnType = th.dataset.columnType;
    var notNull = th.dataset.columnNotNull == 1 ? " NOT NULL" : "";
    if (columnType) {
      columnTypeP.style.display = "block";
      columnTypeP.innerText = `Type: ${columnType.toUpperCase()}${notNull}`;
    } else {
      columnTypeP.style.display = "none";
    }

    var columnDescriptionP = menu.querySelector(".dropdown-column-description");
    if (th.dataset.columnDescription) {
      columnDescriptionP.innerText = th.dataset.columnDescription;
      columnDescriptionP.style.display = "block";
    } else {
      columnDescriptionP.style.display = "none";
    }

    menu.style.position = "absolute";
    menu.style.top = menuTop + 6 + "px";
    menu.style.left = menuLeft + "px";
    menu.style.display = "block";
    menu.classList.add("anim-scale-in");

    // Plugin hook for column actions
    const columnActionsPayload = {
      columnName: th.dataset.column,
      columnNotNull: th.dataset.columnNotNull === "1",
      columnType: th.dataset.columnType,
      isPk: th.dataset.isPk === "1",
    };
    const columnItemConfigs = manager.makeColumnActions(columnActionsPayload);

    const menuList = menu.querySelector("ul");
    columnItemConfigs.forEach((itemConfig) => {
      const existingItems = menuList.querySelectorAll(`li`);
      Array.from(existingItems)
        .filter((item) => item.innerText === itemConfig.label)
        .forEach((node) => node.remove());

      const newLink = document.createElement("a");
      newLink.textContent = itemConfig.label;
      newLink.href = itemConfig.href ?? "#";
      if (itemConfig.onClick) {
        newLink.onclick = itemConfig.onClick;
      }

      const menuItem = document.createElement("li");
      menuItem.appendChild(newLink);
      menuList.appendChild(menuItem);
    });

    // Adjust menu position if needed
    const menuWidth = menu.offsetWidth;
    const windowWidth = window.innerWidth;
    if (menuLeft + menuWidth > windowWidth) {
      menu.style.left = windowWidth - menuWidth - 20 + "px";
    }

    const hook = menu.querySelector(".hook");
    const icon = th.querySelector(".dropdown-menu-icon");
    const iconRect = icon.getBoundingClientRect();
    const hookLeft = iconRect.left - parseFloat(menu.style.left) + 1 + "px";
    hook.style.left = hookLeft;

    const menuRect = menu.getBoundingClientRect();
    if (iconRect.right > menuRect.right) {
      menu.style.left = iconRect.right - menuWidth + "px";
      hook.style.left = menuWidth - 13 + "px";
    }
  }

  // Create and add dropdown icons to table headers
  var svg = document.createElement("div");
  svg.innerHTML = DROPDOWN_ICON_SVG;
  svg = svg.querySelector("*");
  svg.classList.add("dropdown-menu-icon");

  // MODIFIED: Use root element for scoped queries
  var ths = Array.from(root.querySelectorAll(manager.selectors.tableHeaders));
  ths.forEach((th) => {
    if (!th.querySelector("a")) {
      return;
    }
    var icon = svg.cloneNode(true);
    icon.addEventListener("click", onTableHeaderClick);
    th.appendChild(icon);
  });

  // Track cleanup for menu removal
  currentCleanupFunctions.push(() => {
    menu.remove();
  });
};

/**
 * Add x buttons to the filter rows
 * MODIFIED: Accepts root element
 */
function addButtonsToFilterRows(manager, root = document) {
  var x = "X";
  var rows = Array.from(root.querySelectorAll(manager.selectors.filterRows))
    .filter((el) => el.querySelector(".filter-op"));

  rows.forEach((row) => {
    // Skip if already has remove button
    if (row.querySelector('a[aria-label="Remove this filter"]')) {
      return;
    }

    var a = document.createElement("a");
    a.setAttribute("href", "#");
    a.setAttribute("aria-label", "Remove this filter");
    a.style.textDecoration = "none";
    a.innerText = x;
    a.addEventListener("click", (ev) => {
      ev.preventDefault();
      let row = ev.target.closest("div");
      row.querySelector("select").value = "";
      row.querySelector(".filter-op select").value = "exact";
      row.querySelector("input.filter-value").value = "";
      ev.target.closest("a").style.display = "none";
    });
    row.appendChild(a);
    var column = row.querySelector("select");
    if (!column.value) {
      a.style.display = "none";
    }
  });
}

/**
 * Set up datalist autocomplete for filter values
 * MODIFIED: Accepts root element
 */
function initAutocompleteForFilterValues(manager, root = document) {
  function createDataLists() {
    var facetResults = root.querySelectorAll(manager.selectors.facetResults);
    Array.from(facetResults).forEach(function (facetResult) {
      // Skip if datalist already exists
      if (root.querySelector('#datalist-' + facetResult.dataset.column)) {
        return;
      }

      var links = Array.from(
        facetResult.querySelectorAll("li:not(.facet-truncated) a"),
      );
      var datalist = document.createElement("datalist");
      datalist.id = "datalist-" + facetResult.dataset.column;
      links.forEach(function (link) {
        var option = document.createElement("option");
        option.label = link.innerText;
        option.value = link.dataset.facetValue;
        datalist.appendChild(option);
      });
      facetResult.appendChild(datalist);
    });
  }
  createDataLists();

  const changeHandler = function (event) {
    if (event.target.name === "_filter_column") {
      const filterRow = event.target.closest(manager.selectors.filterRows);
      if (filterRow) {
        const filterValue = filterRow.querySelector(".filter-value");
        if (filterValue) {
          filterValue.setAttribute("list", "datalist-" + event.target.value);
        }
      }
    }
  };

  document.body.addEventListener("change", changeHandler);
  currentCleanupFunctions.push(() => document.body.removeEventListener("change", changeHandler));
}

/**
 * Event listener for datasette_init
 * MODIFIED: Handles both initial load and reinit, uses root element
 */
document.addEventListener("datasette_init", function (evt) {
  const { detail } = evt;
  const manager = detail.manager || detail;
  const root = detail.root || document;
  const isReinit = detail.isReinit || false;

  // Clean up previous initialization if this is a reinit
  if (isReinit) {
    cleanupDatasetteTable();
  }

  // Initialize table features
  initDatasetteTable(manager, root);
  addButtonsToFilterRows(manager, root);
  initAutocompleteForFilterValues(manager, root);
});
