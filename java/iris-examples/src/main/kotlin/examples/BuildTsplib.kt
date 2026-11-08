package examples

import dev.gecode.iris.build.FloatField
import dev.gecode.iris.build.Gecode
import dev.gecode.iris.build.IntExpr
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.RecordName
import dev.gecode.iris.build.StringField
import dev.gecode.iris.build.TextualExpr
import dev.gecode.iris.kotlin.build.IrisSpec
import dev.gecode.iris.kotlin.build.OptionalScope0
import dev.gecode.iris.kotlin.build.Scope0
import dev.gecode.iris.kotlin.build.Sections
import dev.gecode.iris.kotlin.build.and
import dev.gecode.iris.kotlin.build.defined
import dev.gecode.iris.kotlin.build.eq
import dev.gecode.iris.kotlin.build.float
import dev.gecode.iris.kotlin.build.ge
import dev.gecode.iris.kotlin.build.int
import dev.gecode.iris.kotlin.build.int2
import dev.gecode.iris.kotlin.build.irisSpec
import dev.gecode.iris.kotlin.build.le
import dev.gecode.iris.kotlin.build.or
import dev.gecode.iris.kotlin.build.range
import dev.gecode.iris.kotlin.build.recordName
import dev.gecode.iris.kotlin.build.rest
import dev.gecode.iris.kotlin.build.size
import dev.gecode.iris.kotlin.build.until
import dev.gecode.iris.kotlin.build.word
import java.nio.file.Path

private const val MAX_DIMENSION = 46_340L
private const val COORDINATE_BOUND = 1e9

/** The handles of one section of numbered coordinates. */
class Coordinates(
    val node: IntField<LongArray?>,
    val x: FloatField<DoubleArray?>,
    val y: FloatField<DoubleArray?>,
)

/** The handles of a TSPLIB file. */
class Tsp(
    val name: StringField<String?>,
    val n: IntField<Long>,
    val edgeWeightType: StringField<String?>,
    val coordinates: Coordinates,
    val weight: IntField<List<LongArray>?>,
    val display: Coordinates,
)

/**
 * A TSPLIB reader. A file gives either an explicit distance matrix or node coordinates, from which
 * the distances follow.
 */
val tsplib: IrisSpec<Tsp> =
    irisSpec("tsplib") {
        comment("COMMENT")
        val name by optionalCapture(rest())
        val type by optionalCapture(word())
        val n by capture(range(2).to(MAX_DIMENSION))
        val edgeWeightType by optionalCapture(word(), name = "edge_weight_type")
        val edgeWeightFormat by optionalCapture(word(), name = "edge_weight_format")
        val nodeCoordType by optionalCapture(word(), name = "node_coord_type")
        val displayDataType by optionalCapture(word(), name = "display_data_type")
        val weight = int2("weight").optional()
        val nodes = recordName("_", "nodes")
        val (coordinates, display) =
            sections(until("EOF")) {
                arm("NAME : ", name, optional = true)
                arm("TYPE : ", type, optional = true)
                arm("DIMENSION : ", n)
                arm("EDGE_WEIGHT_TYPE : ", edgeWeightType, optional = true)
                arm("EDGE_WEIGHT_FORMAT : ", edgeWeightFormat, optional = true)
                arm("NODE_COORD_TYPE : ", nodeCoordType, optional = true)
                arm("DISPLAY_DATA_TYPE : ", displayDataType, optional = true)
                val coordinates = coordinateSection("NODE_COORD_SECTION", n, "", nodes)
                optionalArm("EDGE_WEIGHT_SECTION") {
                    ifThen(!defined(edgeWeightFormat)) { set(edgeWeightFormat, "FULL_MATRIX") }
                    val bound = Gecode.intMax / n
                    read(weight, range(0).to(bound).dim(n).dim(n).layout(edgeWeightFormat))
                }
                val display = coordinateSection("DISPLAY_DATA_SECTION", n, "display_")
                coordinates to display
            }
        ifThen(!defined(edgeWeightType)) { set(edgeWeightType, "EXPLICIT") }

        val (x, y) = coordinates.x to coordinates.y
        ifThen(edgeWeightType eq "EXPLICIT") {
            check(defined(weight), "no EDGE_WEIGHT_SECTION")
            check(size(weight) eq n, "edge weights do not match DIMENSION")
        }
        otherwise {
            check(
                edgeWeightType.oneOf("EUC_2D", "CEIL_2D", "ATT", "GEO"),
                "EDGE_WEIGHT_TYPE must be EXPLICIT, EUC_2D, CEIL_2D, ATT, or GEO",
            )
            check(defined(x), "no NODE_COORD_SECTION")
            check(size(nodes.list()) eq n, "coordinates do not match DIMENSION")
            check(
                (x ge -COORDINATE_BOUND) and
                    (x le COORDINATE_BOUND) and
                    (y ge -COORDINATE_BOUND) and
                    (y le COORDINATE_BOUND),
                "coordinates must be between -1e9 and 1e9",
            )
        }
        Tsp(name, n, edgeWeightType, coordinates, weight, display)
    }

/** Adds the optional section [title] of [n] lines `node x y`, with entry names after [prefix]. */
private fun Sections<Scope0, OptionalScope0>.coordinateSection(
    title: String,
    n: IntField<Long>,
    prefix: String,
    lines: RecordName = recordName(),
): Coordinates =
    optionalArm(title) {
        repeat(n, lines) {
            Coordinates(
                read("${prefix}node", int()),
                read("${prefix}x", float()),
                read("${prefix}y", float()),
            )
        }
    }

/** Whether this is one of [values]: `this == "A" || this == "B"`. */
private fun TextualExpr.oneOf(vararg values: String): IntExpr =
    values.map { this eq it }.reduce { any, next -> any or next }

fun main() {
    val (spec, tsp) = tsplib
    for (example in listOf("explicit-five", "euc-2d-five")) {
        val data = spec.read(Path.of("formats/tsplib/cases/$example/input.txt"))
        println(
            "${data[tsp.name] ?: "unnamed"}: ${data[tsp.n]} cities, ${data[tsp.edgeWeightType]}"
        )
        data[tsp.weight]?.forEach { println("  ${it.contentToString()}") }
        val x = data[tsp.coordinates.x]
        val y = data[tsp.coordinates.y]
        if (x != null && y != null) {
            println("  x ${x.contentToString()}")
            println("  y ${y.contentToString()}")
        }
    }
}
