// Marker state: four variables with empty/unset/zero/zero init. U reifies the
// undone count; B the redo bound; positions are restore targets.
module marker_state {
  var markers: List[{ pre: int, post: int }]
  var u: int
  var b: int
  var at: int

  action init = all {
    markers' = [],
    u' = 0,
    b' = 0,
    at' = 0
  }
}
