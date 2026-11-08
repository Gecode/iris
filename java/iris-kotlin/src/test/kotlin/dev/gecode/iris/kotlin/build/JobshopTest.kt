package dev.gecode.iris.kotlin.build

import dev.gecode.iris.IrisException
import dev.gecode.iris.Params
import dev.gecode.iris.Spec
import dev.gecode.iris.build.Gecode
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.StringField
import java.nio.file.Path
import kotlin.io.path.exists
import kotlin.io.path.listDirectoryEntries
import kotlin.io.path.readText
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows

private const val MSG_DISTINCT = "a job uses a machine twice; each job must use every machine once"
private const val MSG_TOTAL = "the total duration is too large"

/** The handles of a jobshop instance, carried out of the builder. */
class Shop(
    val name: StringField<String?>,
    val nJobs: IntField<Long>,
    val nMachines: IntField<Long>,
    val machine: IntField<List<LongArray>>,
    val duration: IntField<List<LongArray>>,
)

private val jobshop =
    irisBuilder("jobshop") {
        comment("#", "+++")
        instances {
            val name by optionalCapture(word())
            val nJobs by capture(atLeast(1), name = "n_jobs")
            val nMachines by capture(atLeast(1), name = "n_machines")
            val (job, machine, duration) =
                sections(skipOther = true) {
                    arm("instance ", name, optional = true)
                    arm(nJobs, " ", nMachines, last = true) {
                        check(
                            nJobs le (Gecode.intMax - 2) / 2 / nMachines,
                            "there are too many jobs and machines",
                        )
                        repeat(nJobs) {
                            val (machine, duration) =
                                repeat(nMachines) {
                                    read("machine", 0..<nMachines) to read("duration", atLeast(0))
                                }
                            Triple(record, machine, duration)
                        }
                    }
                }
            check(distinct(job[machine]), MSG_DISTINCT)
            check(sum(job) { sum(it[duration]) } le Gecode.intMax, MSG_TOTAL)
            Shop(name, nJobs, nMachines, machine, duration)
        }
    }

/** The running example of `docs/builders.md`, built with the Kotlin builder. */
class JobshopTest {
    @Test
    fun theBuilderPrintsTheRunningExample() {
        assertEquals(
            """
            format jobshop
            comment "#" "+++"
            instances {
              sections skip other {
                "instance {name:word}" optional
                "{n_jobs:1..} {n_machines:1..}" last {
                  check n_jobs <= (gecode.int_max - 2) / 2 / n_machines "there are too many jobs and machines"
                  repeat n_jobs as records_1 {
                    each record_1
                    repeat n_machines {
                      machine: 0..<n_machines
                      duration: 0..
                    }
                  }
                }
              }
              check distinct(record_1.machine) "$MSG_DISTINCT"
              check sum(element_1 in records_1: sum(element_1.duration)) <= gecode.int_max "$MSG_TOTAL"
            }

            """
                .trimIndent(),
            jobshop.source(),
        )
    }

    /** For every case of `formats/jobshop`, the built spec reads what the text spec reads. */
    @Test
    fun theBuiltSpecReadsWhatTheTextSpecReads() {
        val format = Path.of(System.getProperty("iris.repository")).resolve("formats/jobshop")
        val text = Spec.parse(format.resolve("jobshop.iris").readText())
        val (built, shop) = jobshop.build()
        var read = 0
        var failed = 0
        for (case in format.resolve("cases").listDirectoryEntries().sorted()) {
            val input = case.resolve("input.txt").readText()
            val paramsFile = case.resolve("params.json")
            val params =
                if (paramsFile.exists()) {
                    val instance =
                        Regex("\"instance\":\\s*\"([^\"]*)\"").find(paramsFile.readText())
                    Params.of("instance", instance!!.groupValues[1])
                } else {
                    Params.of()
                }
            val expected =
                try {
                    text.read(input, "input", params)
                } catch (error: IrisException) {
                    val again = assertThrows<IrisException> { built.read(input, "input", params) }
                    assertEquals(error.code(), again.code())
                    failed++
                    continue
                }
            val data = built.read(input, "input", params)
            assertEquals(expected.toJson(), data.toJson(), case.toString())
            val name: String? = data[shop.name]
            val jobs: Long = data[shop.nJobs]
            val machines: Long = data[shop.nMachines]
            val rows: List<LongArray> = data[shop.machine]
            assertEquals(expected.find("name")?.asString(), name)
            assertEquals(expected["n_jobs"].asLong(), jobs)
            assertEquals(expected["n_machines"].asLong(), machines)
            assertEquals(jobs, rows.size.toLong())
            for ((k, row) in rows.withIndex()) {
                assertArrayEquals(expected["machine"][k].longs(), row)
                assertArrayEquals(expected["duration"][k].longs(), data[shop.duration][k])
            }
            read++
        }
        assertEquals(2, read)
        assertEquals(1, failed)
    }
}
