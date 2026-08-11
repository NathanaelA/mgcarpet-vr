package rust.jniminhelper;

import android.app.Activity;
import android.os.Bundle;
import android.content.Intent;
import android.util.Log;
import android.os.Handler;
import android.os.Looper;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.view.Gravity;
import android.graphics.Color;


public class PermActivity extends Activity {
    static final String EXTRA_PERM_ARRAY = "rust.jniminhelper.perm_array";
    static final String EXTRA_TITLE = "rust.jniminhelper.perm_activity_title";

    private String[] permissions;
    private TextView label;

    // to be registered in native code
    private native void nativeOnRequestPermissionsResult(String[] permissions, int[] grantResults);

	@Override
	protected void onCreate(Bundle savedInstanceState) {
	    super.onCreate(savedInstanceState);
	    Intent intent = this.getIntent();
	    this.permissions = intent.getStringArrayExtra(EXTRA_PERM_ARRAY);
	    createScreen();
	}

    private void createScreen() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setPadding(40, 40, 40, 40);          // optional padding
        layout.setBackgroundColor(Color.BLACK);     // optional background

        // Create the text box (EditText)
        TextView label = new TextView(this);
          label.setText("If you see this without a permission dialog\nThe data needs to be installed..");
          label.setTextSize(22);
          label.setTextColor(Color.WHITE);
          label.setGravity(Gravity.CENTER);
          label.setPadding(40, 40, 40, 40);
          label.setBackgroundColor(Color.BLACK);
          this.label = label;

          layout.addView(label);

        // Set the whole layout as the activity content
        setContentView(layout);
    }

	@Override
	protected void onStart() {
	    super.onStart();
	    Intent intent = this.getIntent();
	    String title = intent.getStringExtra(EXTRA_TITLE);
	    this.setTitle(title);
	}

	@Override
	protected void onResume() {
	    super.onResume();
	    this.requestPermissions(this.permissions, 0);
	}

    @Override
    public void onRequestPermissionsResult(int requestCode,
        String[] permissions, int[] grantResults)
    {
        Log.d("mgcarpet", "Permissions result: " + String.join(", ", permissions));
        if (grantResults.length == 2 && grantResults[0] == 0 && grantResults[1] == 0) {
            // All permissions granted
            this.label.setText("Permissions granted.\nPlease close and restart the app.");
                 new Handler(Looper.getMainLooper()).postDelayed(() -> {
                        finishAffinity();
                        System.exit(0);
                    }, 2500);

        } else {
             this.nativeOnRequestPermissionsResult(permissions, grantResults);
        }
    }

}