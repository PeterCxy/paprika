// Avatar animation on hover
// Unfortunately for best result, this can only be done with JS
let avatar = document.querySelector(".sidebar .avatar img");
avatar.onmouseover = (ev) => {
  ev.target.className = "animate";
};
avatar.onanimationend = (ev) => {
  ev.target.className = "";
};