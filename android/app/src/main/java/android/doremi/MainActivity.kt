package android.doremi

import android.doremi.ui.theme.DoremiTheme
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        //enableEdgeToEdge()
        setContent {
            DoremiTheme() {
                // TODO: read notes from app's folder
                ViewNotes(NotesViewModel(TestData.notes))
            }
        }
    }
}

data class Note(val name: String, val tags: List<String>, val body: String, val ctime: Long = 0)

class NotesViewModel(
    val notes: List<Note>
) : ViewModel() {
    // var notes = mutableStateListOf<Note>().also { it.addAll(notes) }

    // eg: one_word_match "two-word match" n:name t:"two-word tag" "b:three-word body content"
    var filter by mutableStateOf("")
}

@Composable
fun ViewNote(note: Note) {
    // TODO:
    //  - prettier (everything)
    //  - editing inline
    //  - clickable links in body
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 4.dp)
    ) {
        // TODO: move into viewmodel or something. there's only one note open at a time
        var isOpen by remember { mutableStateOf(false) }
        val surfaceColor by animateColorAsState(
            if (isOpen) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
        )

        Column(
            modifier = Modifier
                .clickable { isOpen = !isOpen }
                .padding(12.dp)
        ) {
            Text(
                text = note.name,
                color = MaterialTheme.colorScheme.secondary,
                style = MaterialTheme.typography.titleSmall
            )
            if (note.tags.isNotEmpty()) {
                Text(
                    text = note.tags.joinToString(", "),
                    color = MaterialTheme.colorScheme.secondary,
                    style = MaterialTheme.typography.labelSmall
                )
            }
            Spacer(modifier = Modifier.height(4.dp))
            Surface(
                shape = MaterialTheme.shapes.medium,
                color = surfaceColor,
                modifier = Modifier
                    .animateContentSize()
                    .fillMaxWidth()
            ) {
                Text(
                    text = note.body,
                    modifier = Modifier.padding(all = 4.dp),
                    maxLines = if (isOpen) Int.MAX_VALUE else 1,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }
    }
}

@Composable
fun ViewNotes(vm: NotesViewModel) {
    Surface(modifier = Modifier.fillMaxSize()) {
        Column() {
            // TODO:
            //  - disappear after scroll down
            TextField(
                value = vm.filter,
                onValueChange = { vm.filter = it },
                label = { Text("Search") },
                modifier = Modifier.fillMaxWidth()
            )
            // TODO:
            //  - group by month/show month headers
            LazyColumn() {
                items(vm.notes) { note ->
                    ViewNote(note)
                }
            }
        }
    }
}

object TestData {
    // Sample conversation data
    val notes = listOf(
        Note(
            "Lexi",
            listOf("Pippo"),
            "Test...Test...Test..."
        ),
        Note(
            "Lexi",
            listOf("Pippo", "pluto"),
            """List of Android versions:
            |Android KitKat (API 19)
            |Android Lollipop (API 21)
            |Android Marshmallow (API 23)
            |Android Nougat (API 24)
            |Android Oreo (API 26)
            |Android Pie (API 28)
            |Android 10 (API 29)
            |Android 11 (API 30)
            |Android 12 (API 31)""".trim()
        ),
        Note(
            "Lexi",
            listOf(),
            """I think Kotlin is my favorite programming language.
            |It's so much fun!""".trim()
        ),
        Note(
            "Lexi",
            listOf("Franco", "Programming", "Rust"),
            "Searching for alternatives to XML layouts..."
        ),
        Note(
            "Lexi",
            listOf(),
            """Hey, take a look at Jetpack Compose, it's great!
            |It's the Android's modern toolkit for building native UI.
            |It simplifies and accelerates UI development on Android.
            |Less code, powerful tools, and intuitive Kotlin APIs :)""".trim()
        ),
        Note(
            "Lexi",
            listOf(),
            "It's available from API 21+ :)"
        ),
        Note(
            "Lexi",
            listOf(),
            "Writing Kotlin for UI seems so natural, Compose where have you been all my life?"
        ),
        Note(
            "Lexi",
            listOf(),
            "Android Studio next version's name is Arctic Fox"
        ),
        Note(
            "Lexi",
            listOf(),
            "Android Studio Arctic Fox tooling for Compose is top notch ^_^"
        ),
        Note(
            "Lexi",
            listOf(),
            "I didn't know you can now run the emulator directly from Android Studio"
        ),
        Note(
            "Lexi",
            listOf(),
            "Compose Previews are great to check quickly how a composable layout looks like"
        ),
        Note(
            "Lexi",
            listOf(),
            "https://www.google.com"
        ),
        Note(
            "Lexi",
            listOf(),
            "Have you tried writing build.gradle with KTS?"
        ),
    )
}
