package examples

import dev.gecode.iris.build.Gecode
import dev.gecode.iris.build.IntField
import dev.gecode.iris.kotlin.build.IrisSpec
import dev.gecode.iris.kotlin.build.eq
import dev.gecode.iris.kotlin.build.int2
import dev.gecode.iris.kotlin.build.irisSpec
import dev.gecode.iris.kotlin.build.le
import dev.gecode.iris.kotlin.build.range
import dev.gecode.iris.kotlin.build.recordName
import java.nio.file.Path

private const val MAX_NODES = 1000L

/** The handles of a time-window problem: distances and a time window for each node. */
class TimeWindows(
    val n: IntField<Long>,
    val dist: IntField<List<LongArray>>,
    val ready: IntField<LongArray>,
    val due: IntField<LongArray>,
)

/**
 * A reader for the travelling salesman problem with time windows: a distance matrix and a time
 * window for each node, with the depot as node 0.
 */
val tsptw: IrisSpec<TimeWindows> =
    irisSpec("tsptw") {
        comment("#")
        val n = read("n", range(2).to(MAX_NODES))
        val dist = int2("dist")
        read(dist, range(0).to(Gecode.intMax / 2 / n).dim(n).dim(n))

        val time = defineType("time", range(0).to(Gecode.intMax / 2))
        val nodes = recordName("node", "nodes")
        val (ready, due) = repeat(n, nodes) { read("ready", time) to read("due", time) }

        check(nodes[ready] le nodes[due], "the time window of a node is invalid")
        check(nodes.list().at(0)[ready] eq 0, "the depot (node 0) must open at time 0")
        TimeWindows(n, dist, ready, due)
    }

fun main() {
    val (spec, windows) = tsptw
    val data = spec.read(Path.of("formats/tsptw/cases/three-nodes/input.txt"))
    println("${data[windows.n]} nodes")
    val opens = data[windows.ready]
    val closes = data[windows.due]
    for ((k, distances) in data[windows.dist].withIndex()) {
        println("node $k: open ${opens[k]}..${closes[k]}, distances ${distances.contentToString()}")
    }
}
