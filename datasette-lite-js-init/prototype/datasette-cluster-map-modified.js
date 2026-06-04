// Modified datasette-cluster-map.js for SPA/reinit support
// Original used DOMContentLoaded; now uses datasette_init event

const clusterMapCSS = `
dl.cluster-map-dl {
    font-size: 1.1em;
}
dl.cluster-map-dl dt {
    font-weight: bold;
}
dl.cluster-map-dl dd {
    margin: 0px 0 0 0.7em;
}
dl.cluster-map-dl dd span {
    color: #aaa;
    font-size: 0.8em;
}
button.cluster-map-button {
    color: #fff;
    background-color: #007bff;
    border-color: #007bff;
    vertical-align: middle;
    cursor: pointer;
    border: 1px solid blue;
    padding: 0.3em 0.8em;
    font-size: 0.6rem;
    line-height: 1;
    border-radius: .25rem;
}
`;

// Track current map for cleanup
let currentMapInstance = null;
let currentMapElement = null;

/**
 * Cleanup previous map instance
 */
function cleanupClusterMap() {
  if (currentMapInstance) {
    try {
      currentMapInstance.remove();
    } catch (e) {
      console.warn('Failed to remove map:', e);
    }
    currentMapInstance = null;
  }
  if (currentMapElement) {
    currentMapElement.remove();
    currentMapElement = null;
  }
  // Remove progress div if exists
  const progressDiv = document.querySelector('.cluster-map-progress');
  if (progressDiv) {
    progressDiv.remove();
  }
}

/**
 * Main initialization - CHANGED from DOMContentLoaded to datasette_init
 */
document.addEventListener("datasette_init", (evt) => {
  const { detail } = evt;
  const root = detail.root || document;
  const isReinit = detail.isReinit || false;

  // Clean up previous map on reinit
  if (isReinit) {
    cleanupClusterMap();
  }

  // Only execute on table, query and row pages
  // MODIFIED: Check within root element, not whole document
  const body = root.querySelector ? root : document;
  const pageBody = document.querySelector("body.table,body.row,body.query");

  // For SPA mode, check for table presence instead of body class
  const hasTable = root.querySelector("table.rows-and-columns");

  if (!pageBody && !hasTable) {
    return;
  }

  // Does it have Latitude and Longitude columns?
  let columns = Array.prototype.map.call(
    root.querySelectorAll("table.rows-and-columns th"),
    (th) => (th.getAttribute("data-column") || th.textContent).trim()
  );

  let latitudeColumn = null;
  let longitudeColumn = null;
  const latColName = window.DATASETTE_CLUSTER_MAP_LATITUDE_COLUMN || 'latitude';
  const lonColName = window.DATASETTE_CLUSTER_MAP_LONGITUDE_COLUMN || 'longitude';

  columns.forEach((col) => {
    if (col.toLowerCase() === latColName.toLowerCase()) {
      latitudeColumn = col;
    }
    if (col.toLowerCase() === lonColName.toLowerCase()) {
      longitudeColumn = col;
    }
  });

  if (latitudeColumn && longitudeColumn) {
    // Load dependencies and then add the map
    const loadDependencies = (callback) => {
      // Check if already loaded
      if (window.L && window.L.markerClusterGroup) {
        callback();
        return;
      }

      let loaded = [];
      function hasLoaded() {
        loaded.push(this);
        if (loaded.length == 3) {
          callback();
        }
      }

      let stylesheet = document.createElement("link");
      stylesheet.setAttribute("rel", "stylesheet");
      stylesheet.setAttribute("href", datasette.leaflet.CSS_URL);
      stylesheet.onload = hasLoaded;
      document.head.appendChild(stylesheet);

      let stylesheet2 = document.createElement("link");
      stylesheet2.setAttribute("rel", "stylesheet");
      stylesheet2.setAttribute(
        "href",
        datasette.cluster_map.MARKERCLUSTER_CSS_URL
      );
      stylesheet2.onload = hasLoaded;
      document.head.appendChild(stylesheet2);

      import(datasette.leaflet.JAVASCRIPT_URL).then(() => {
        import(datasette.cluster_map.MARKERCLUSTER_URL).then(hasLoaded);
      });
    };

    loadDependencies(() => addClusterMap(latitudeColumn, longitudeColumn, root));
  }
});

// ... rest of helper functions remain the same as original ...

const clusterMapEscapeHTML = (s) =>
  s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");

const clusterMapMarkerContent = (row) => {
  let popup = row.popup;
  if (popup) {
    if (typeof popup === "string") {
      try {
        popup = JSON.parse(popup);
      } catch (e) {
        popup = {};
      }
    }
    if (popup.image || popup.title || popup.description || popup.link) {
      let html = [];
      if (popup.link) {
        html.push('<a href="' + clusterMapEscapeHTML(popup.link) + '">');
      }
      if (popup.title) {
        html.push(
          "<p><strong>" + clusterMapEscapeHTML(popup.title) + "</strong></p>"
        );
      }
      if (popup.image) {
        html.push(
          '<img style="max-width: 100%" src="' +
            clusterMapEscapeHTML(popup.image) +
            '"'
        );
        if (popup.alt) {
          html.push(' alt="' + clusterMapEscapeHTML(popup.alt) + '"');
        }
        html.push(">");
      }
      if (popup.description) {
        html.push(
          '<p style="text-decoration: none; color: black;">' +
            clusterMapEscapeHTML(popup.description) +
            "</p>"
        );
      }
      if (popup.link) {
        html.push("</a>");
      }
      return html.join("");
    }
  }

  const dl = document.createElement("dl");
  Object.keys(row).forEach((key) => {
    const dt = document.createElement("dt");
    dt.appendChild(document.createTextNode(key));
    const dd = document.createElement("dd");
    let value = row[key];
    let label = value;
    let extra = null;
    if (typeof value === "object") {
      if (
        value !== null &&
        value.label !== undefined &&
        value.value !== undefined
      ) {
        label = value.label;
        extra = document.createElement("span");
        extra.appendChild(document.createTextNode(" " + value.value));
      } else {
        label = JSON.stringify(value);
      }
    }
    dd.appendChild(document.createTextNode(label));
    if (extra) {
      dd.appendChild(extra);
    }
    dl.appendChild(dt);
    dl.appendChild(dd);
  });
  return (
    '<dl class="cluster-map-dl" style="height: 200px; overflow: auto">' +
    dl.innerHTML +
    "</dl>"
  );
};

/**
 * Add the cluster map to the page
 * MODIFIED: Accepts root element for scoped queries
 */
const addClusterMap = (latitudeColumn, longitudeColumn, root = document) => {
  let keepGoing = false;

  // Only add CSS once
  if (!document.querySelector('style[data-cluster-map-css]')) {
    let style = document.createElement("style");
    style.setAttribute('data-cluster-map-css', 'true');
    style.innerText = clusterMapCSS;
    document.head.appendChild(style);
  }

  function isValidLatitude(latitude) {
    latitude = parseFloat(latitude);
    if (isNaN(latitude)) {
      return false;
    }
    return latitude >= -90 && latitude <= 90;
  }

  function isValidLongitude(longitude) {
    longitude = parseFloat(longitude);
    if (isNaN(longitude)) {
      return false;
    }
    return longitude >= -180 && longitude <= 180;
  }

  const loadMarkers = (path, map, markerClusterGroup, progressDiv, count) => {
    count = count || 0;
    return fetch(path)
      .then((r) => r.json())
      .then((data) => {
        let markerList = [];
        data.rows.forEach((row) => {
          if (isValidLatitude(row[latitudeColumn]) && isValidLongitude(row[longitudeColumn])) {
            let markerContent = clusterMapMarkerContent(row);
            let marker = L.marker(
              L.latLng(row[latitudeColumn], row[longitudeColumn])
            );
            marker.bindPopup(markerContent);
            markerList.push(marker);
          }
        });
        count += data.rows.length;
        markerClusterGroup.addLayers(markerList);
        map.fitBounds(markerClusterGroup.getBounds());

        let next_url = data.next_url;
        if (next_url && location.protocol == "https:") {
          next_url = next_url.replace(/^https?:/, "https:");
        }
        let total = data.count || data.filtered_table_rows_count;
        if (next_url) {
          let percent = ` (${Math.round((count / total) * 100 * 100) / 100}%)`;
          let button = document.createElement("button");
          button.classList.add("cluster-map-button");
          if (keepGoing) {
            button.innerHTML = "pause";
            button.addEventListener("click", () => {
              keepGoing = false;
            });
          } else {
            button.innerHTML = "load all";
            button.addEventListener("click", () => {
              keepGoing = true;
              loadMarkers(next_url, map, markerClusterGroup, progressDiv, count);
            });
          }
          progressDiv.innerHTML = `Showing ${count.toLocaleString()} of ${total.toLocaleString()}${percent} `;
          progressDiv.appendChild(button);
        } else {
          progressDiv.innerHTML = "";
        }
        if (next_url && keepGoing) {
          return loadMarkers(next_url, map, markerClusterGroup, progressDiv, count);
        }
      });
  };

  let el = document.createElement("div");
  el.style.width = "100%";
  el.style.height = "500px";

  let tiles = L.tileLayer(
    window.DATASETTE_CLUSTER_MAP_TILE_LAYER || 'https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png',
    window.DATASETTE_CLUSTER_MAP_TILE_LAYER_OPTIONS || {}
  );

  let map = L.map(el, {
    zoom: 13,
    layers: [tiles],
  });

  // Store reference for cleanup
  currentMapInstance = map;
  currentMapElement = el;

  const container = window.DATASETTE_CLUSTER_MAP_CONTAINER;
  if (container && root.querySelector(container)) {
    root.querySelector(container).appendChild(el);
  } else {
    let table =
      root.querySelector(".table-wrapper") ||
      root.querySelector("table.rows-and-columns");
    if (table) {
      table.parentNode.insertBefore(el, table);
    }
  }

  let progressDiv = document.createElement("div");
  progressDiv.classList.add('cluster-map-progress');
  progressDiv.style.marginBottom = "2em";
  el.parentNode.insertBefore(progressDiv, el.nextSibling);

  let markerClusterGroup = L.markerClusterGroup({
    chunkedLoading: true,
    maxClusterRadius: 50,
  });
  map.addLayer(markerClusterGroup);

  // Build the JSON path - handle both normal and SPA modes
  let basePath = location.pathname;
  let queryString = location.search;

  // In Datasette-lite SPA mode, the path might be in the hash
  if (location.hash && location.hash.startsWith('#/')) {
    const hashPath = location.hash.slice(1);
    const [path, qs] = hashPath.split('?');
    basePath = path;
    queryString = qs ? '?' + qs : '';
  }

  let path = basePath + ".json" + queryString;
  const qs = "_size=max&_labels=on&_extra=count&_extra=next_url&_shape=objects";
  if (path.indexOf("?") > -1) {
    path += "&" + qs;
  } else {
    path += "?" + qs;
  }

  loadMarkers(path, map, markerClusterGroup, progressDiv, 0);
};
