// Avatar animation on hover
// Unfortunately for best result, this can only be done with JS
let avatar = document.querySelector(".sidebar .avatar img");
avatar.onmouseover = (ev) => {
  ev.target.className = "animate";
};
avatar.onanimationend = (ev) => {
  ev.target.className = "";
};

// Trigger progress bar when loading new page
window.onbeforeunload = (ev) => {
  document.getElementsByClassName("loading-progress")[0].className = "loading-progress force-visible";
};

window.onload = function() {
  let content = document.getElementsByClassName("content");
  if (content.length == 0) return;

  let level = 0;
  let maxLevel = 0;
  let toc = "";
  content[0].querySelectorAll("h1, h2, h3, h4, h5").forEach((elem) => {
    let openLevel = parseInt(elem.tagName.toLowerCase().replace("h", ""));
    if (openLevel > level) {
      toc += (new Array(openLevel - level + 1)).join("<ul>");
    } else if (openLevel < level) {
      toc += (new Array(level - openLevel + 1)).join("</ul>");
    }

    level = openLevel;
    if (level > maxLevel) maxLevel = level;

    let anchor = elem.getAttribute("id");
    let titleText = elem.innerText;
    toc += "<li><a href=\"#" + anchor + "\">" + titleText + "</a></li>";
  });

  if (level) {
    toc += (new Array(level + 1)).join("</ul>");
  }

  if (maxLevel > 1) {
    document.getElementsByClassName("toc")[0].innerHTML = toc;
    document.getElementsByClassName("toc-wrapper")[0].className = "toc-wrapper"; // remove hidden

    // Get rid of ul layers that have only one ul child
    removeTrivialUlLayer(document.getElementsByClassName("toc")[0]);

    var curAnchorLink = null;
    window.onscroll = (ev) => {
      let anchor = findClosestAnchor(content[0].querySelectorAll("h1, h2, h3, h4, h5"));
      let name = anchor.getAttribute("id");
      let tocLink = document.querySelector("a[href=\"#" + name + "\"").parentElement;
      if (tocLink != curAnchorLink) {
        tocLink.className = "current";
        if (curAnchorLink != null) {
          curAnchorLink.className = "";
        }
        curAnchorLink = tocLink;
      }
    };

    window.onscroll();
  }
};

function removeTrivialUlLayer(elem) {
  let children = elem.getElementsByTagName("ul");
  if (elem.childNodes.length == 1 && children.length == 1) {
    // Every child is a ul
    elem.innerHTML = children[0].innerHTML;
    removeTrivialUlLayer(elem);
  } else {
    for (const child of children) {
      removeTrivialUlLayer(child);
    }
  }
}

// <https://stackoverflow.com/questions/10642587/finding-closest-anchor-href-via-scrolloffset>
// findPos : courtesy of @ppk - see http://www.quirksmode.org/js/findpos.html
var findPos = function (obj) {
  var curleft = 0,
    curtop = 0;
  if (obj.offsetParent) {
    curleft = obj.offsetLeft;
    curtop = obj.offsetTop;
    while ((obj = obj.offsetParent)) {
      curleft += obj.offsetLeft;
      curtop += obj.offsetTop;
    }
  }
  return [curleft, curtop];
};

var findClosestAnchor = function (anchors) {
  var sortByDistance = function (element1, element2) {
    var pos1 = findPos(element1),
      pos2 = findPos(element2);

    // vect1 & vect2 represent 2d vectors going from the top left extremity of each element to the point positionned at the scrolled offset of the window
    var vect1 = [
      window.scrollX - pos1[0],
      window.scrollY - pos1[1]
    ],
      vect2 = [
        window.scrollX - pos2[0],
        window.scrollY - pos2[1]
      ];

    // we compare the length of the vectors using only the sum of their components squared
    // no need to find the magnitude of each (this was inspired by Mageek’s answer)
    var sqDist1 = vect1[0] * vect1[0] + vect1[1] * vect1[1],
      sqDist2 = vect2[0] * vect2[0] + vect2[1] * vect2[1];

    if (sqDist1 < sqDist2) return -1;
    else if (sqDist1 > sqDist2) return 1;
    else return 0;
  };

  // Convert the nodelist to an array, then returns the first item of the elements sorted by distance
  return Array.prototype.slice.call(anchors).sort(sortByDistance)[0];
};
